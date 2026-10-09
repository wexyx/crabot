# 聊天记录与 Agent 上下文

聊天记录只存于项目知识索引，不读写小时日志文件。

## 存储与身份

- 项目聊天使用 group ID；执行 run ID 只标识一轮任务。
- 管理聊天使用项目内固定的管理会话。
- 索引位于 `CRABOT_DATA_DIR/knowledge/<project UUID>/graph.db`。
- 每条事件保存完整内容、聊天 ID、递增序号和时间，包括工具事件、完成与重置标记。
- 聊天记录与序号在同一数据库事务中提交，写入失败不会只推进序号。

## 每个 Agent 独立的上下文

上下文按“项目＋聊天＋Agent”隔离：读取该 Agent 最近一次摘要，再按序读取摘要覆盖位置之后的全部聊天记录。没有摘要时，从最近的上下文重置点开始读取。不会固定只取最近 N 轮，也不会因分页只加载前 1000 条。

每个 Agent 按自己的上下文长度压缩。新摘要必须保留此前摘要的信息；摘要覆盖上限在构建输入时固定，不会覆盖生成期间新到达、尚未读到的消息。一个 Agent 的摘要不改变其他 Agent 的上下文，也不删除或隐藏界面的原始聊天记录。

`find(target=history)` 搜索历史；`find(target=history)` 按序号回查原文，包括已被摘要覆盖的记录。`compact` 触发压缩。

## Web 读取接口

```text
GET /v1/repl/{project}/chats/{admin|group ID}/history
GET /v1/repl/{project}/chats/{admin|group ID}/history?before=123&limit=300
GET /v1/repl/{project}/chats/{admin|group ID}/history?after=123&limit=300
GET /v1/repl/{project}/chats/{admin|group ID}/events?after=123
```

返回 `events`、`active_run`、`has_more`。默认最近 300 条，最多 1000 条；before 向前翻页，after 向后续读。SSE 使用事件序号继续读取。接口不接受文件名或本机文件路径，不提供旧日志文件下载入口。

旧聊天日志不自动导入。更名后的命令为 `crabot`、默认数据目录为 `~/.crabot`，环境变量前缀为 `CRABOT_`；不兼容旧产品名。

## 索引内存

历史索引按存储命名空间独立打开，默认缓冲池上限为 1024 MiB，按需分配，并非启动就占满。可在实例 `.agent.env` 中设置 `CRABOT_HISTORY_BUFFER_MIB=2048`（64–8192 MiB，重启生效）。这是每个索引的内存上限，不是历史记录容量限制；较低的上限可能无法处理大段工具输出的 checkpoint。

自动 checkpoint 的 WAL 阈值为 1 MiB，给事务和索引维护预留内存；查询连接用完释放，空闲索引句柄采用有界缓存，关闭时释放缓冲池。聊天原文仍完整保留在索引中，不通过删记录释放内存。数据库已确认提交但后续 checkpoint 失败时保留成功状态并输出警告，不重复追加同一批记录。
