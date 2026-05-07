use anyhow::{Context, Result, anyhow};
use rand::{
    SeedableRng,
    distr::{Alphabetic, Distribution},
    rng,
    rngs::SmallRng,
};
use reqwest::Client;
use scraper::{Html, Selector};
use serde::Deserialize;
use std::fs;
use tokio_retry::{
    Retry,
    strategy::{ExponentialBackoff, jitter},
};

const MAIN_URL: &str = "https://xsijishe.net";

#[derive(Deserialize, Debug)]
struct Account {
    username: String,
    password: String,
}

#[derive(Debug)]
struct LoginParams {
    formhash: String,
    referer: String,
}

#[derive(Debug)]
struct CheckInParams {
    href: String,
    referer: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let config_dir = dirs::config_dir()
        .expect("config dir should be known on the platform")
        .join("sijishe");
    std::fs::create_dir_all(&config_dir)?;

    let config_path = config_dir.join("accounts.json");
    let accounts_data = fs::read_to_string(&config_path)
        .context(format!("Failed to read {}", config_path.display()))?;
    let accounts: Vec<Account> = serde_json::from_str(&accounts_data)
        .context(format!("Failed to parse {}", config_path.display()))?;

    if accounts.is_empty() {
        println!("No accounts found in {}", config_path.display());
        return Ok(());
    }

    println!("⚙️ Config loaded from {}", config_path.display());

    for account in accounts.iter() {
        println!("========================================");
        println!("🚀 Starting check-in for user: {}", account.username);

        match process_account(account).await {
            Ok(_) => println!("✅ Finished processing for {}", account.username),
            Err(e) => eprintln!("❌ Error processing {}: {:?}", account.username, e),
        }
    }

    Ok(())
}

async fn process_account(account: &Account) -> Result<()> {
    // Create a reqwest client with cookie store enabled
    let client = Client::builder()
        .cookie_store(true)
        .referer(false)
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/114.0 Safari/537.36")
        .build()?;

    // 1. Get login parameters
    let params = get_login_params(&client).await?;
    println!("📝 Fetched login params: formhash={}", params.formhash);

    // 2. Login
    login(&client, account, &params).await?;

    // 3. Get check-in parameters
    let params = get_check_in_params(&client).await?;
    println!("📝 Fetched check-in params: href={}", params.href);

    // 4. Do Check-in
    do_check_in(&client, &params).await?;

    // 5. Print User Info
    print_user_info(&client).await?;

    Ok(())
}

async fn get_login_params(client: &Client) -> Result<LoginParams> {
    let referer = format!("{}/home.php?mod=space", MAIN_URL);
    let res = client
        .get(&referer)
        .header(reqwest::header::REFERER, format!("{}/", MAIN_URL))
        .send()
        .await?
        .text()
        .await?;

    let document = Html::parse_document(&res);

    // Parse formhash
    let formhash_selector = Selector::parse("input[name='formhash']").unwrap();
    let formhash = document
        .select(&formhash_selector)
        .next()
        .and_then(|el| el.attr("value"))
        .unwrap_or("")
        .to_string();

    Ok(LoginParams { formhash, referer })
}

async fn login(client: &Client, account: &Account, params: &LoginParams) -> Result<()> {
    let login_url = format!(
        "{}/member.php?mod=logging&action=login&loginsubmit=yes&handlekey=login&loginhash=L{}&inajax=1",
        MAIN_URL,
        get_random_string(4)
    );

    let password_md5 = format!("{:x}", md5::compute(account.password.as_bytes()));

    let payload = [
        ("formhash", params.formhash.as_str()),
        ("referer", params.referer.as_str()),
        ("username", account.username.as_str()),
        ("password", password_md5.as_str()),
        ("questionid", "0"),
        ("answer", ""),
    ];

    let res = client
        .post(&login_url)
        .header(reqwest::header::REFERER, params.referer.as_str())
        .form(&payload)
        .send()
        .await?
        .text()
        .await?;

    if res.contains("欢迎您回来") {
        println!("🎉 [Success] Login successful!");
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "Login failed. Response snippet: {:.100}",
            res
        ))
    }
}

async fn get_check_in_params(client: &Client) -> Result<CheckInParams> {
    let referer = format!("{}/k_misign-sign.html", MAIN_URL);
    let href_selector = Selector::parse("a[id='JD_sign']").unwrap();

    Retry::spawn(
        ExponentialBackoff::from_millis(10).map(jitter).take(3),
        || async {
            let res = client
                .get(&referer)
                .header(reqwest::header::REFERER, format!("{}/", MAIN_URL))
                .send()
                .await?
                .text()
                .await?;

            let document = Html::parse_document(&res);

            // Parse formhash
            let href = document
                .select(&href_selector)
                .next()
                .and_then(|el| el.attr("href"))
                .ok_or(anyhow!(
                    "Failed to get check in href (maybe already checked in)"
                ))?
                .to_string();

            Ok(href)
        },
    )
    .await
    .map(|href| CheckInParams { href, referer })
}

async fn do_check_in(client: &Client, params: &CheckInParams) -> Result<()> {
    println!("⏳ Executing check-in operation...");

    let check_in_url = format!("{}/{}", MAIN_URL, params.href);
    let res = client
        .get(&check_in_url)
        .header(reqwest::header::REFERER, params.referer.as_str())
        .send()
        .await?
        .text()
        .await?;

    if res.contains("今日已签") || res.contains("您今天已经签到过了") {
        println!("✅ Already checked in today.");
    } else if res.contains("签到成功") || res.contains("CDATA") {
        println!("🎉 Check-in successful!");
    } else {
        println!(
            "⚠️ Check-in failed or returned unexpected response: {:.100}",
            res
        );
    }

    Ok(())
}

async fn print_user_info(client: &Client) -> Result<()> {
    println!("🔎 Fetching user info...");

    let check_in_page_url = format!("{}/k_misign-sign.html", MAIN_URL);
    let html = client
        .get(&check_in_page_url)
        .header(reqwest::header::REFERER, format!("{}/", MAIN_URL))
        .send()
        .await?
        .text()
        .await?;
    let document = Html::parse_document(&html);

    let get_val = |id: &str| -> String {
        let sel = Selector::parse(&format!("input[id='{}']", id)).unwrap();
        document
            .select(&sel)
            .next()
            .and_then(|el| el.attr("value"))
            .unwrap_or("Unknown")
            .to_string()
    };

    let qiandao_num = get_val("qiandaobtnnum");
    let lxdays = get_val("lxdays");
    let lxtdays = get_val("lxtdays");
    let lxlevel = get_val("lxlevel");
    let lxreward = get_val("lxreward");

    let sel = Selector::parse("li[class='nexmemberinfostwos'] > p").unwrap();
    let total_reward = document
        .select(&sel)
        .next()
        .map(|el| el.inner_html())
        .unwrap_or("Unknown".to_string());

    println!("签到排名：{}", qiandao_num);
    println!("签到等级：Lv.{}", lxlevel);
    println!("连续签到：{} 天", lxdays);
    println!("签到总数：{} 天", lxtdays);
    println!("签到奖励：{}", lxreward);
    println!("总积分：{}", total_reward);

    Ok(())
}

async fn buy(client: &Client, tid: &str) -> Result<()> {
    println!("👀 Fetching thread info: {} ...", tid);

    let refer = format!("{}/thread-{}-1-1.html", MAIN_URL, tid);
    client
        .get(&refer)
        .header(reqwest::header::REFERER, format!("{}/", MAIN_URL))
        .send()
        .await?;

    let buy_page_url = format!(
        "{}/jnpar_pansell-pay.html?tid={}&pid=&infloat=yes&handlekey=jnpar_pay_win1&inajax=1&ajaxtarget=fwin_content_jnpar_pay_win1",
        MAIN_URL, tid
    );
    let fragment = client
        .get(&buy_page_url)
        .header(reqwest::header::REFERER, &refer)
        .send()
        .await?
        .text()
        .await?;
    let fragment = Html::parse_fragment(&fragment);
    let cdata = fragment
        .select(&Selector::parse("root").unwrap())
        .next()
        .ok_or(anyhow!("Failed to get CDATA"))?
        .inner_html();
    let form = cdata
        .strip_prefix("<!--[CDATA[")
        .ok_or(anyhow!("Failed to strip prefix `<!--[CDATA[`"))?
        .strip_suffix("]]&gt;")
        .ok_or(anyhow!("Failed to strip suffix `]]&gt;"))?;
    let form = Html::parse_fragment(form);

    let get_val = |name: &str| -> String {
        let sel = Selector::parse(&format!("input[name='{}']", name)).unwrap();
        form.select(&sel)
            .next()
            .and_then(|el| el.attr("value"))
            .unwrap_or("Unknown")
            .to_string()
    };
    let formhash = get_val("formhash");
    let handlekey = get_val("handlekey");
    let tid = get_val("tid");
    let pid = get_val("pid");

    println!("📝 Fetched thread params: formhash={}", formhash);

    println!("💰 Buying thread {} ...", tid);
    let buy_url = format!("{}/plugin.php?id=jnpar_pansell:pay", MAIN_URL);
    let payload = [
        ("formhash", formhash.as_str()),
        ("handlekey", handlekey.as_str()),
        ("tid", tid.as_str()),
        ("pid", pid.as_str()),
        ("submit", "true"),
    ];
    let resp = client
        .post(&buy_url)
        .header(reqwest::header::REFERER, &refer)
        .form(&payload)
        .send()
        .await?;
    let redirected_url = resp.url();

    if redirected_url.to_string() == refer {
        println!("✅ Buy thread {} successfully", tid);
    } else {
        return Err(anyhow!(
            "Failed to buy thread {}: reward not enough or bought before",
            tid
        ));
    }

    Ok(())
}

fn get_random_string(len: usize) -> String {
    let mut rng = SmallRng::from_rng(&mut rng());
    std::iter::repeat_with(|| Alphabetic.sample(&mut rng) as char)
        .take(len)
        .collect()
}
