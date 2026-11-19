# 执行器工作目录清理边界

日期：2026-11-18。范围：T06 的进程退出后目录核验与清理错误；不包括跨重启自动回收。

现有两项工作目录回归先通过，再修改 `crates/core/src/protocol.rs`。本轮仅修复所有权边界及清理错误传播，不新增依赖，不调整任务并发、浏览器连接或原始业务断言。

1. 保留独立随机目录、Unix 0700、目录替换不删除、目录内符号链接不影响外部内容的行为。
2. 用持有的目录句柄固定身份。Unix 比较设备/inode；Windows 用 Win32 `GetFileInformationByHandleEx(FileIdInfo)` 比较卷及 128 位文件 ID，并拒绝 reparse point。未知平台或身份检查失败均保留目录。
3. 所有权标记只允许有界普通文件；拒绝标记替换、符号链接、超限及缺失。显式清理返回错误，不能仅记警告后报告成功；析构仍作保守兜底。
4. 进程树确定退出后才能清理目录；取消/失败保留最初错误及独立清理失败。不得用目录名、PID、复制标记推断跨重启所有权。
5. 核验正常移除、替换目录/标记保留、外部链接隔离、异常引擎退出和重复终止。Windows 目录身份必须在真实 Windows CI 通过，不能用本机 Unix 测试替代。

跨重启遗留目录仍需后续受控诊断/恢复设计，T06 本轮保持未勾选。没有可靠进程与目录身份时保留并诊断，不扫描临时目录批量删除。

完整回归发现 `app.rs` 的历史重试测试偶发失败：测试自己持有的裸文件锁只靠关闭释放，并行启动进程可在 exec 前继承同一 open-file description。生产 `AssetGuard` 已明确 unlock。本轮补测试内的继承句柄 fixture 和明确 unlock，保持断言不变；不修改历史写入策略。

Win32 参考：[GetFileInformationByHandleEx](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getfileinformationbyhandleex)、[FILE_ID_INFO](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_id_info)。
