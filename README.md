# About

司机社自动签到，批量购买帖子的命令行工具

```sh
Sijishe CLI

Usage: sijishe <COMMAND>

Commands:
  checkin     Check in
  buy         Buy a thread
  completion  Generate shell completion scripts
  help        Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

# Config

json配置文件内容（可以先运行一次，报错信息会提示工具尝试加载配置文件的位置）
```json
[
    {
        "username": "name1",
        "password": "plain text"
    },
    {
        "username": "name2",
        "password": "plain text"
    }
]
```

# Usage

- 每个子命令可以通过`-f`参数，使用正则表达式过滤用户名，确定执行操作的账号。
- `buy`子命令的`thread-id`可以从帖子的 url 中看到，比如 `https://xsijishe.net/thread-888888-1-1.html`，`thread-id`就是中间的`888888`

## 具体运行效果：
### 签到：
```sh
sijishe checkin
⚙️ Config loaded from /home/xxx/.config/sijishe/accounts.json
👥 Accounts to process after filtering: xxx
========================================
🚀 Starting check-in for user: xxx
📝 Fetched login params: formhash=xxxx
🎉 [Success] Login successful!
📝 Fetched check-in params: href=plugin.php?id=k_misign:sign&operation=qiandao&formhash=xxxx&format=empty
⏳ Executing check-in operation...
🎉 Check-in successful!
🔎 Fetching user info...
签到排名：xxx
签到等级：Lv.x
连续签到：x 天
签到总数：x 天
签到奖励：xxx
总积分：xxxx
✅ Finished processing for xxx
```
### 购买：
```sh
sijishe buy -y -f 'xxx' 6xxxx9
⚙️ Config loaded from /home/xxx/.config/sijishe/accounts.json
👥 Accounts to process after filtering: xxx
========================================
🚀 Starting buy for user: xxx
📝 Fetched login params: formhash=xxxx
🎉 [Success] Login successful!
👀 Fetching thread info: 6xxxx9 ...
🍌 Parsing subject: 明...强）
✅ Already bought thread 明...强）
✈️ Info:
  我用夸克网盘给你分享了...，点击链接或复制整段内容，打开「夸克APP」即可获取。
  /~...:/
  链接：<a href="https://pan.quark.cn/s/..." target="_blank">点击下载</a>
  提取码：...
  我用夸克网盘给你分享了...，点击链接或复制整段内容，打开「夸克APP」即可获取。
  /~...:/
  链接：<a href="https://pan.quark.cn/s/..." target="_blank">点击下载</a>
  提取码：...

✅ Finished processing for xxx
```

# 下载安装

[release](https://github.com/annosijishe/sijishe_auto_checkin/releases)

标准的做法是解压后放在 `~/.local/bin` 中。

# 定时任务

systemd-timer 参考配置

`/etc/systemd/system/check_in.service`
```ini
[Unit]
Description=Daily check in
After=network-online.target

[Service]
Type=oneshot
User=xxx
ExecStart=/home/xxx/.local/bin/sijishe checkin
```
`/etc/systemd/system/check_in.timer`
```ini
[Unit]
Description=Run check in daily

[Timer]
OnCalendar=*-*-* 00:20:00
AccuracySec=1min
Persistent=true

[Install]
WantedBy=timers.target
```

启用
```sh
systemctl daemon-reload
systemctl enable check_in.timer
```

查看日志
```sh
systemctl status check_in.service
# 或者
journalctl -b0 --user -t sijishe -f
```
