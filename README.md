# 城通网盘下载工具（学习项目）

这是一个基于 Rust 的学习项目，将会根据作者的需求维护。欢迎批评不足之处。

**当前多文件下载功能有问题！无法并发下载！**

**程序不支持破解限速！**

近期存在批量下载城通网盘文件的需求，但找不到合适的解析 & 下载工具，本着熟悉 Rust 语言和解决批量下载需求的目的，编写本程序。
作者在开发过程中参考了开源项目 [qinlili23333/ctfileGet](https://github.com/qinlili23333/ctfileGet) 调用接口的相关代码，在此表示感谢！

程序支持两种模式：类似于 curl、wget 的直接模式与守护模式。守护模式是将一个程序实例作为下载器，而其他程序实例作为它的客户端，避免程序多开。

在开发过程中，为减少代码复杂度，单线程 + 异步的下载方式。

## 版本依赖

作者有更新的习惯，当前版本为：

```shell
$ rustc --version
rustc 1.71.0 (8ede3aae2 2023-07-12)
```

请注意，部分目录硬编码了 UNIX/Linux 路径，因此它在 Windows 系统上可能无法编译或正常使用。

## 使用方法

```shell
$ ./ctfile-rs help
Download file from ctfile.com via CLI.

Usage: ctfile-rs <COMMAND>

Commands:
  daemon    
  parse     
  download  
  list      
  help      Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

**作为守护进程启动**：

```shell
$ ./ctfile-rs daemon [--listen <addr>]
```
默认的监听地址为 `localhost:7735`。

**列出守护进程的任务**：

```shell
$ ./ctfile-rs list [--daemon <addr>]
```

以表格的形式查看 daemon 的任务及状态，输出结束后程序会退出。如需更新，可以使用 `watch` 命令监控。


**解析链接**（暂不支持 VIP 链接，可能出现错误行为）：

```shell
$ ./ctfile-rs parse <URL> [--password <password>] [--token <token>]
```

**直接下载**：

```shell
$ ./ctfile-rs download <URL> [--password <password>] [--token <token>] [--daemon <addr>]
```

在前者的基础上，下载文件到当前目录，下载过程将以进度条的形式显示。

如果指定了 `--daemon` 参数，文件将下载到 `/tmp` 下，程序在任务提交后即退出。


## TODO

- [ ] 守护进程在无任务时自动关闭；
- [ ] 直接模式下，通过添加多个参数下载多个文件；
- [ ] 取消下载任务；
- [ ] 优化 JSON 结果解析部分的代码，不太满意当前的 `serde_json`；
- [ ] 支持 VIP 文件的提示。

## 开源协议

仅供学习和交流使用。