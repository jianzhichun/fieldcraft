# FieldCraft 帧流式渲染服务：补丁设计（首次构建前，不编译 Rust）

目标：为 FieldCraft fork 增加一个**进程内渲染服务**。它在无窗口状态下运行真实的 `ModelApp` 界面，把整窗（面板、工具栏、对话框、3D 视口）画到离屏纹理，按脏矩形编码后经 loopback TCP 流给宿主；宿主的输入以紧凑消息回传，服务端翻译成 egui 事件注入同一份 UI；模型的 `engine.execute`/`engine.batch` 与宿主的 `ui.operations {cancel}` 走同一条连接、同一个 `Session`。首次构建只求把协议、输入、任务与只读四条链路跑通，性能数字留给测量。

证据：`path:line` 均相对 `/mnt/hdd/fieldz/fieldcraft`（分支 `fieldz/sc-0.48.1`，`git f3a0e73`）。结构参照 `/home/dev/workspace/dsh-plugin-solidz/docs/stream/design.md` §§1–3，只借结构，不沿用其产品命名。标记：**[V]** 在源码中读到（本次核查或四份侦察报告带行号）；**[I]** 推断；**[E]** 估算，首次构建必须确认。**本设计未编译、未运行、未连接过真实控制通道。**

## 0. 结论

- 服务是**新增 app** `apps/fieldz-render-server`。对既有文件的必要改动只有 engine 的只读 hook（1 个新文件 + 3 处插入）、一个 `EngineError` 变体，以及（仅在没有 GPU 的兜底路径）`capture.rs` 的一次重构；控制语义、事件注入、截图、任务调度全部复用 `fieldcraft-ui-egui` 已有的公开 API。
- 与参照设计的关键差异：本 fork **没有 `crates/control`，也没有 `Protocol` trait**；`grep` 只命中 objc2 的 `ProtocolObject` [V]。真正可复用的是 `fieldcraft_ui_egui::control::handle(&mut ModelApp, &egui::Context, &ControlRequest) -> Outcome`（`crates/ui-egui/src/control.rs:234`）与 `ControlRequest`（`:23-35`），二者已在库中且公开 [V]。所以参照设计的 E1 在这里不是“把 server 搬进 crate”，而是“新 app 自带带帧的传输，`call` 直接进 `handle`”。
- 传输：loopback TCP，`u32` 长度前缀 + JSON 头 + 二进制载荷，一条连接；首个 `hello` 必须带 token。
- 帧：64×64 瓦片差分得到脏矩形；小矩形原样 RGBA，大矩形 JPEG 分条并行编码，PNG 只用于整帧/快照。`png` 0.18 是 workspace 依赖 [V `Cargo.toml:62`]；`image` 0.25.10 已在 `Cargo.lock:2856`，加为直接依赖只为 JPEG。
- 输入：线上是紧凑 `input` 消息，服务端转成 `ui.move/click/drag/scroll/key/text/drop` 调用 `control::handle`，落到公开的 `ModelApp::synthetic`（`crates/ui-egui/src/lib.rs:490`）[V]，由 `raw_input_hook`（`:2426`）每帧注入。输入映射不重写，沿用 `control.rs:557-624` [V]。
- `engine.batch` 在服务端逐步循环（首错即停、无回滚）；`ui.operations {cancel}` 提供 **120 s 单步预算**与 **5 s 整轮停止确认**，底层用 `start_job`/`cancel_loading` [V `files.rs:1031,1061`]。`job.cancel` 单独不够：它只停一个协作式后台任务，从不停 UI 线程上的同步工作（`control.rs:260-264`）[V]。
- 只读：进程级标志；在 `Session::execute` 的 `find_command` 之后拒绝，并在命令已改文档时把结果降为 `Disabled`，由既有错误路径回退、记失败、不重放（`crates/engine/src/lib.rs:435,508-516,538`；`crash.rs:1007-1019`）[V]。

## 1. 渲染服务

### 1.1 位置，以及对既有文件的改动

新 app `apps/fieldz-render-server/`（包名与二进制名 `fieldz-render-server`）。workspace 的 `members = ["crates/*", "apps/*", "xtask"]`（`Cargo.toml:3`）[V] 自动收编，根清单无需为成员改动；`Cargo.lock` 会变化。

对既有文件的**完整**改动清单：

| # | 文件:行 | 改什么 | 为什么 |
|---|---|---|---|
| E1 | `crates/engine/src/lib.rs:90` | 在 `EngineError::Context { .. }` 之后加 `Disabled { reason: String, command: String }` | 只读拒绝需要可区分的结果。所有 `EngineError` 匹配都有兜底（`:95-118`、`:120-127`、`:128-137`；`ui-egui/lib.rs:745-754`），加变体安全 [V]；建议同时在 `shown`（`:128-137`）加一条中文文案 [I]。 |
| E2 | 新建 `crates/engine/src/read_only.rs`；`crates/engine/src/lib.rs:34` 附近加 `pub mod read_only;` | 进程级 `Mutex<Option<String>>`；`reason()`、`set(reason)`、`blocked(id)` | 必须**进程级**而非 `thread_local`：长命令在 `Session` 的克隆上跑于 worker 线程（`files.rs:651-670`、`:819-831`），线程局部标志到不了那里 [V 克隆与线程；I 由线程边界推出]。 |
| E3 | `crates/engine/src/lib.rs:435-436` | `find_command` 成功之后、`usage::Timer::start` 之前插入只读拒绝 | 该点之前不建计时器、不进 `run_command`、不写 journal 的 `run` 行（`:436`、`:446-447`、`:469`）[V]。 |
| E4 | `crates/engine/src/lib.rs:508-509` | `catch_unwind` 归一化之后、`if let Some((mut before, ..))` 之前，把“改了文档的 Model 结果”改成 `Err(Disabled)` | 复用 `:515-516` 的 `restore(before)` 撤销；`:538` 的 `crash::command_done` 记 `ok:false`；`crash.rs:1007-1019` 只把 `ok` 项放入 replay [V]。条件与 `:517` 相同，须写成 `before.as_ref()`，因 `:509` 会 move。 |
| E5（条件） | `crates/ui-egui/src/capture.rs:75-129` | 从 `capture()` 内部拆出 `pub fn capture_frame(&mut self, size: CaptureSize, events: Vec<egui::Event>) -> Result<ColorImage, String>`：单帧、先 `raw_input_hook`、再 `run_ui`/tessellate/CPU 栅格化 | **仅当首次构建无法构造 `RenderState` 时启用**。`capture()` 自己不跑 `raw_input_hook`，且换入无 GPU 的 `Viewport`（`capture.rs:82`、`:94-114`），因此既不能注入输入、也拿不到 GPU 视口 [V]。 |
| E6 | `Cargo.toml` 的 `[workspace.dependencies]` | 加 `image = { version = "0.25", default-features = false, features = ["jpeg"] }` | JPEG 编码。`image` 0.25.10 已作为传递依赖在锁文件中 [V `Cargo.lock:2856`]，新增只是直接边。 |

**不改**：`ModelApp::run/run_script/start_job/cancel_loading/logic/ui/set_wgpu/raw_input_hook`、公开的 `synthetic`、`ModelApp::capture`、`control::handle`、`ControlRequest` 均已 `pub`（`lib.rs:490,596,717,734,830,2260,2426,2453`；`files.rs:1031,1061`；`control.rs:23-51,234`）[V]。`apps/fieldcraft/src/control_server.rs` **不搬**：它是 JSON-lines 专用，与帧传输无关，搬进库也用不上（这是与参照设计 E1 的实质差异）。UI 的事件注入与截图完成都发生在 `logic()` 里（`lib.rs:2266`、`:2599-2635`），新 app 每帧调用它即可 [V]。

### 1.2 进程内结构（按职责分文件）

`src/main.rs` 组合根（命令行、环境变量、装配、错误退出）· `src/wire.rs` **协议**：长度前缀分帧、serde 消息类型、读写线程 · `src/host.rs` **主机循环**：每帧 `RawInput` → `run_ui` → tessellate → 绘制 → 读回 → 编码 → 发送，以及节奏 · `src/gpu.rs` **捕获**：无窗口 `RenderState`、离屏纹理、egui-wgpu 绘制通道 · `src/readback.rs` 256 字节对齐的 `copy_texture_to_buffer` + map · `src/diff.rs` 64×64 瓦片差分与矩形合并 · `src/encode.rs` raw/JPEG 分条/PNG、rayon 并行 · `src/input.rs` **输入注入**：紧凑消息 → `ui.*` 调用表 · `src/engine.rs` **引擎桥**：`call` → `ControlRequest` → `control::handle`；`engine.batch` · `src/ops.rs` **任务/取消**：操作登记、120 s 预算、5 s 停止确认 · `src/policy.rs` 只读原因。

线程模型：**循环线程**独占 `ModelApp` 与 `egui::Context`（`Services` 等非 `Send` 状态都在这里）[I]；**读线程**解析入站消息，把请求送进一个 `std::sync::mpsc`；**写线程**（或一把写锁）在信用额度内写帧与回复。循环线程只在这个 channel 上等待，超时取最近的 repaint 期限。

依赖（均已在树内）：`fieldcraft-ui-egui`、`fieldcraft-engine`、`fieldcraft-gpu`、`fieldcraft-render`、`egui`、`egui-wgpu`、`wgpu`、`serde`/`serde_json`、`pollster`、`png`、`rayon`（`Cargo.lock:4461`）[V]；新增 `image`（JPEG）。不依赖 `eframe`/`winit`：`ui-egui` 自身也不依赖它们（`crates/ui-egui/Cargo.toml:1-40`）[V]。

### 1.3 无窗口设备与离屏绘制

- 本仓库**没有**构造 `egui_wgpu::RenderState` 的公开助手；唯一的 `RenderState` 来自 eframe（`apps/fieldcraft/src/main.rs:355`）[V]。服务必须自己建：上游 0.36.2 的 `RenderState::create(config, instance, None, options)` 在无 surface 时目标格式取 `vec![Rgba8Unorm]` [V 上游 0.36.2 `lib.rs:199-203,265-289`]；但本 fork 用 git rev 钉 0.37（`Cargo.toml:174-176`），该源码不在盘上 [V]，所以“同一入口在 0.37 仍存在”是 **[E]**，列为开放问题 1。
- 适配器可选两条：`RenderState::create` 自带选择；或先用 `fieldcraft_gpu` 的 `select::adapter(policy)` 打开设备再交给 `RenderState`（若该 rev 有 `from_device` 一类入口）[I]。`select.rs` 已按平台给后端：mac/iOS Metal，Windows DX12|Vulkan，其余 Vulkan|GL [V `crates/gpu/src/select.rs:92-100`]。
- 目标：`Rgba8Unorm`、无深度、**整窗 pass 用 1 采样**（3D 视口自己画自己的深度与 MSAA，见下）。桌面窗口用的是 `DEPTH_BITS=24`、`MSAA=4`（`apps/fieldcraft/src/main.rs:27-30`）[V]，但那是窗口 surface 的参数，不能照搬。
- 视口管线：`app.set_wgpu(&rs, 24, rs.msaa_samples, &ctx)`（`lib.rs:717-730`）[V]；视口的 egui 绘制回调与目标无关（`gpu.rs:104-112`），且已有测试证明它能画进离屏 pass 并读回（`gpu.rs:130-175`）[V]。因此整窗 GPU 路径**不需要**改 `capture.rs`。
- 离屏纹理：`RENDER_ATTACHMENT | COPY_SRC`，设备像素尺寸，`resize` 时重建；读回走 256 字节对齐的 staging buffer + `map_async` + `device.poll(Wait)`。树内已有同型实现可抄：`fieldcraft_gpu::capture::Capturer::offscreen`（`crates/gpu/src/capture.rs:152-249`）[V]，但它只画模型、不画 egui；egui 侧的上传/绘制/读回要自己写。
- **流式路径不要用 `ModelApp::capture`**：它自建 context、不跑 `raw_input_hook`、并换入无 GPU 的 `Viewport`（`capture.rs:82,94-114`）[V]。`capture` 只保留给 `ui.capture`（`control.rs:652-664`）与 `ui.screenshot` 的迟到兜底（`lib.rs:2623-2628`）[V]。

### 1.4 ModelApp 的装配

顺序如下，全部是公开入口：

1. `let ctx = egui::Context::default();` `theme::apply(&ctx, dark)`（`crates/ui-egui/src/theme.rs:530-537`）[V]；字体/图标由 `apply` 内部安装（`:535-536`）[V]。
2. `ModelApp::new(Session::default())`（`lib.rs:596`）[V]；`Services` 默认即全 `None`（`files.rs:59-72`）[V]，不调用 `with_services` 也能跑；`with_control` **不用**，请求由本 app 直接送进 `control::handle`（`drain_control` 在无 channel 时是空操作，`lib.rs:2572`）[V]。
3. 可选进程级注册：`fieldcraft_lbm_gpu::register()`、`fieldcraft_mesh_gpu::register()`、`fieldcraft_ui_egui::register_picture_drawer()`（`apps/fieldcraft/src/main.rs:282-287`；`lib.rs:587-593`）[V]。不注册时图片命令回退 CPU，UI 本身不受影响。
4. `app.set_wgpu(&rs, 24, rs.msaa_samples, &ctx)`（`lib.rs:717-730`）[V]，在建第一帧之前同步完成。
5. `policy::set(args.read_only)`（§1.8）；不启动 `Autosaver`（`apps/fieldcraft/src/main.rs:388` 会启动，本 app 不照搬）[V]。
6. 不进：崩溃安装、系统菜单、更新、恢复 UI、原生文件对话框（`Services` 全 `None`）[V/I]。

### 1.5 一帧

循环线程每帧执行，顺序与 eframe 的 native 集成一致（仅保留与离屏有关的部分）[I 结构，各调用点 V]：

1. **RawInput**：`screen_rect = Rect(0,0,w,h)`（单位为 point，即 CSS px）、`viewports[ROOT].native_pixels_per_point = Some(dpr)`、`time = Some(app.now 或单调时钟)`、`focused`、`max_texture_side = device.limits().max_texture_side`；`events` 先为空，随后 `app.raw_input_hook(&ctx, &mut raw)`（`lib.rs:2426-2450`）[V]，把 `synthetic` 里的控制/输入事件按“每帧至多一个指针事件或一段按键”的规则注入。
2. **Pass**：`let out = ctx.run_ui(raw, |ui| { app.logic(ctx); app.ui(ui); });`。`logic` 设时间、应用主题、`drain_control`（本 app 不用）、`poll_loading`、截图轮询（`lib.rs:2260-2268`、`:2596-2635`）[V]；`ui` 画整窗（`:2453-2480`）[V]。
3. **Tessellate**：`ctx.tessellate(out.shapes, out.pixels_per_point)`（同 `capture.rs:120`）[V]。
4. **绘制**：`renderer.update_texture` 处理 `textures_delta.set`；`renderer.update_buffers(...)` 取回绘制回调的命令缓冲；一个 render pass 先清成主题面板色，再 `renderer.render(&mut pass, &primitives, &screen)`。视口回调在这个 pass 里画 3D；`free_texture` 处理 `textures_delta.free`。这就是上游 egui-wgpu 的 `Painter::paint_frame` 去掉 surface 后的样子 [I，首次构建确认]。
5. **读回**：`copy_texture_to_buffer` 到三条缓冲之一（v1 同步 `poll(Wait)`，最低延迟）[E]；按 256 字节行对齐解包成 RGBA。
6. **差分**：与“上一次已发送帧”按 64×64 瓦片比较，合并且合并成矩形；`resize`/首帧/`full` 请求时整帧。
7. **编码**：按 §2.3 逐矩形编码，组一个 `frame` 消息。
8. **发送**：在信用额度内写出；无变化、不可见、无信用时不编码（损伤累积）。

### 1.6 帧节奏

- 循环阻塞在 channel 上，超时 = 最近一个待办 repaint；唤醒源：`input`/`resize`/`visible`/`ack`/`call`/`stop`、`ctx.set_request_repaint_callback` 的回调 [I]、上一帧的 `repaint_delay` [I]。
- 至多 60 帧/秒；一次 pass 内到达的事件合并到下一次（egui 本就接受多个 `PointerMoved`）。
- 没有变化不编码；不可见不编码；无信用不编码（脏区累积）。但这些情况下 pass 仍要跑，任务回复、定时器、后台任务结果都靠它。
- 空闲时理论上可以完全不跑 pass；但 `poll_loading` 在有任务时会请求重绘（`lib.rs:2267-2268`）[V]，所以有任务时自然按 50 ms 轮询。

### 1.7 控制通道：engine.execute / batch 与 ui.operations

线上 `call` 形如 `{"t":"call","id":N,"method":"engine.execute","params":{...},"op":"turn/step"}`。`engine.rs` 把 `method/params` 塞进 `ControlRequest::new(method, params, tx)`（`control.rs:23-35`）[V]，调用 `control::handle(app, &ctx, &req)`，再按 `Outcome` 处理（`control.rs:42-51`）[V]：

- `Done(v)` → 回 `reply {id, result:v}`。
- `Pending` → 回复延后，`Sender` 已进入 `loading.waiters`（`control.rs:224-232`）[V]，任务结束/取消时统一发（`files.rs:1093-1095`、`:1065-1068`）[V]。
- `Screenshot {path}` → 交给每帧的截图轮询，完成时经 `Sender` 回（`lib.rs:2599-2635`）[V]。

**`engine.execute`**：完全复用现有语义——`is_long(id)` 的走 `start_job`，`"wait": false` 立刻回 `{"job": n}`，否则延后；其余走 UI 线程的 `app.run`（`control.rs:243-257`；`is_long` 见 `files.rs:175-182`）[V]。长任务类别是求解、网格、打开、分区（`files.rs:175-182`）[V]。

**`engine.batch`**（新方法，服务端实现）：逐步循环，每步调用上面的 `engine.execute`；首错即停，返回 `{ok:false, completed:i, failed:{...}, results:[...]}`；不设回滚，已完成的步骤保持生效（与 MCP 的 batch 一致，`crates/mcp/src/tools.rs:597-623`，也与 `Session::run_script` 的首错即停一致，`engine/src/lib.rs:587`）[V]。上限 `MAX_STEPS = 10_000`（`tools.rs:12`）[V]。若某步是长任务，batch 在该步挂起等待其 reply，再继续下一步。

**`ui.operations {op, cancel}`**（新方法）：

- `cancel` 为具体 `op` 时只停该操作；为 `"agent"` 时停整轮（所有未完成 op + 未开始的 batch 步骤）。
- 取消动作落到 `app.cancel_loading()`（`files.rs:1061`）[V]，协作式任务会向所有 waiter 回 `{"ok":false,"error":"cancelled: …","cancelled":true}`（`files.rs:1065-1068`）[V]。
- **120 s 单步预算**：每步起记时间戳。后台任务在循环线程每帧检查，超过即 `cancel_loading()` 并回 `{ok:false, error:"step budget 120 s exceeded", code:"budget"}`。UI 线程上的同步命令无法抢占（循环被 `session.execute` 占住），只能在返回后回 `{over_budget:true, elapsed_ms}` [I]。
- **5 s 整轮停止确认**：收到整轮 cancel 后，读/写线程在 ≤5 s 内回 `{stopped:bool, at, elapsed_ms}`。后台任务“总会回答”（`apps/fieldcraft/src/control_server.rs:178-180` 的注释与代码、`files.rs:1093-1095`）[V]，所以通常在下一帧就确认；若循环卡在同步命令里，读线程独立写出 `{stopped:false, reason:"ui thread busy"}` [I/E]。这就是“停止确认”而不是“已停止”的诚实语义。
- 为什么不用 `job.cancel` 充当：它只停一个协作式后台任务，从不停 UI 线程的同步工作，也没有“整轮”概念（`control.rs:258-264`）[V]。

### 1.8 只读 hook

服务以 `--read-only <原因>` 或 `FIELDCRAFT_READ_ONLY` 设置原因，进程启动时 `read_only::set(...)`。

- **前置拒绝**（E3，`lib.rs:435-436`）：`file.save`、`file.save_as`、`edit.undo`、`edit.redo`、`file.recover`（`cmd/file.rs:44,54,74`；`cmd/edit.rs:36,46`）[V]，以及文档替换/无撤销快照的 Session 类写命令：`file.new`、`file.open`、`file.sample`（`cmd/file.rs:24,34,94`）、`deck.read`、`deck.write`（`cmd/deck.rs:17,27`）、`results.export.*`、`results.render.save`、`results.export.animation`、`view.image.save`、`mesh.export.stl`、`file.recovery.discard`（`cmd/results.rs:79,89`；`cmd/render.rs:28,48`；`cmd/image.rs:22`；`cmd/mesh.rs:106`；`cmd/file.rs:84`）[V]。这些 Session 命令**没有** `before` 快照（`:467` 只对 `CommandKind::Model` 取），无法回退，只能前置拒绝 [V]。
- **事后降级**（E4，`lib.rs:508-509`）：Model 命令若在只读期间改了文档（`before.document.is_some() || self.document() != *doc || self.differs_from(before)`，与 `:517` 同式），把结果换成 `Disabled`；`:515-516` 的 `restore(before)` 撤销它，`:538` 记 `ok:false`，`crash.rs:1007-1019` 不会把它放进 replay [V]。`geometry.open` 是 Model 类（`cmd/geometry.rs:29`），走这条；`solve.run` 虽是 Model 类但不改文档（`:28`），因此**放行**，结果仍可写入 [V]。
- **为什么必须在 engine**：UI 有 7 处直接 `session.execute` 绕过 `app.run`（`lib.rs:1081,1096,1188,1209,1215,2085`；`report.rs:101`）[V]，`engine.script`（`control.rs:263`）与 batch 也走 `execute` [V]；放在 UI 或服务层会漏。
- **可观察结果**：被拒的 `file.save` 不会触发 `mark_saved`，`is_dirty()` 仍为真（`lib.rs:757-763`、`:881-891`）[V]，宿主状态栏可据此显示“只读，未保存”。
- **绕过点**：`Autosaver` 直接写恢复槽，不经 `execute`（`autosave.rs:37-52`）[V]；服务不启动它即可。另可把 `Session.file_access`（`lib.rs:375,838-846`）设成宿主给的写根，作为第二道路径约束 [V 存在；I 是否启用由宿主决定]。

## 2. 线协议 v1

### 2.1 传输与分帧

- **TCP loopback，服务端监听** `127.0.0.1:0`（不占固定端口），启动后向 stdout 打一行 `ready {"v":1,"port":N,"pid":N}`；`--port`/`FIELDCRAFT_RENDER_PORT` 可覆盖 [I]。只接受一条连接；第二条拒绝（`MAX_CONNECTIONS = 1`）[I]。这与控制通道同为 loopback（`control_server.rs:38`）[V]。
- **认证**：首条消息必须 `hello {"v":1,"token":"…"}`，token 取自 `FIELDCRAFT_RENDER_TOKEN`，常量时间比较，失败即关连接。控制通道今天“除绑定 loopback 外没有认证”（`control_server.rs:26-28`）[V]，且会忽略多余的顶层字段（`:163-170`）[V]；本服务把 token 作为协议的一部分，避免被误连 [I]。不落 token 文件、不落端口文件。
- **分帧**：`u32 BE header_len`（≤ 64 KiB）| JSON 头 | 载荷。载荷上限 128 MiB（4K×4K RGBA 为 64 MiB；对照控制通道的行上限 16 MiB 与 MCP 回复上限 256 MiB——`control_server.rs:25`、`backend.rs:107`）[V/E]。一个读线程解析入站，一个写锁串行化出站，帧与回复不会交错。
- 为什么不用 stdio：服务与宿主是不同进程，TCP 与控制通道一致、跨平台一条路径；也不与可能的标准输出日志打架 [I]。

### 2.2 消息一览

| 方向 | 类型 |
|---|---|
| 服务端 → 宿主 | `ready`、`frame`*、`cursor`*、`status`*、`reply`、`bye`、`error` |
| 宿主 → 服务端 | `hello`、`input`、`resize`、`visible`、`ack`、`call`、`stop` |

`frame`/`cursor` 高频，走二进制载荷；其余是 JSON 头内联。`status` 携带 `revision`（取自 `session.revision()`，`crates/engine/src/lib.rs:755`）与 `busy`（`files.rs:804`），宿主据此发现用户编辑，不需要订阅方法 [I]。流控：起始 2 个信用，每发一帧用 1 个，每收一个 `ack {seq}` 还 1 个；服务端从不丢帧，无信用时延迟并累积脏区，脏矩形因此始终正确 [I]。

`call` 覆盖 `engine.execute`、`engine.batch`、`engine.script`、`engine.commands`、`job.*`、`ui.operations`、以及既有的 `ui.move/click/drag/scroll/key/text/drop`、`ui.select`、`ui.selection`、`ui.capture`、`ui.screenshot`、`ui.render` 等 [V 既有方法见 `control.rs:242-696`]。

### 2.3 帧：脏矩形、编码与尺寸

参考尺寸：**R1** 面板 1280×800 CSS、DPR 2（Retina）= 2560×1600 = 4.10 Mpx = 16.4 MB RGBA；**R2** 同尺寸 DPR 1 = 1.02 Mpx = 4.1 MB。

| 阶段 | R2 | R1 | 依据 |
|---|---|---|---|
| UI pass（`logic`+`ui`） | 2–6 ms | 2–6 ms | [E] 按 point 不按像素 |
| tessellate + GPU 绘制 | 1–3 ms | 2–5 ms | [E] |
| 读回（copy + map） | ~1 ms | 2–4 ms | [E] |
| 瓦片差分 | <1 ms | 2–3 ms | [E] 16 MB memcmp |
| JPEG 整帧（`image`，单核） | 15–35 ms | 70–140 ms | [E] |
| JPEG 8 条并行（rayon） | 3–6 ms | 10–20 ms | [E] |
| PNG（fast deflate）整帧 | 15–30 ms | 40–80 ms | [E] |
| 每整帧 JPEG 字节（q80） | 100–150 KB | 400–600 KB | [E] |

v1 规则：

- 矩形 ≤ 65 536 px（打字时的输入框、光标）：`raw` 原样 RGBA，不编码；
- 更大矩形（轨道旋转时的视口）：`jpeg` q≈80，按 ≤256 行分条，每条一个 rect，`rayon` 并行编码，宿主可并发解码；
- `png`：首帧与 `resize` 后的整帧用它（无损、便于宿主首屏对齐）；200 ms 无输入后再把视口区域按 q95 4:4:4 重发一次（JPEG，清晰化文字与边线）。PNG 用现成的 `fieldcraft_render::png`（定义 `crates/render/src/raster.rs:501`，经 `crates/render/src/lib.rs:23-26` 公开重导出，签名为 `(w, h, rgba)`，可直接编码整窗）[V]；
- 脏矩形：64×64 瓦片与上次**已发送**帧比较，按行贪心合并；`full:true` 时宿主清屏重画；
- 尺寸：以宿主 `resize {w,h,dpr}` 为准，设备像素 = `round(css*dpr)`；上限沿用 `MAX_SIDE = 8192`、`MAX_PIXELS = 4096*4096`（`capture.rs:22-25`）[V]，`scale` 允许 0.5–4.0（`capture.rs:26`）[V]；超限时降低实际 dpr 并在头里回 `dprUsed`，帧仍按实际像素发。

帧头示例：`{"t":"frame","seq":N,"w":2560,"h":1600,"dpr":2.0,"dprUsed":2.0,"full":false,"rects":[{"x":..,"y":..,"w":..,"h":..,"codec":"jpeg","off":0,"len":12345,"rows":256}, …]}`，载荷是按 `off/len` 拼接的各矩形字节。

### 2.4 输入映射

| 线上 `input` 事件 | 转成的控制调用 | 落到 egui |
|---|---|---|
| `move {x,y}` | `ui.move` | `PointerMoved`（`control.rs:557-570`）[V] |
| `button {x,y,button,down,double}` | `ui.click` | `PointerMoved` + `PointerButton` press/release（`:557-570`）[V] |
| `wheel {x,y,dx,dy,unit}` | `ui.scroll` | `MouseWheel { unit: Line, phase: Move, source: Wheel }`（`:584-597`）[V] |
| `key {key,physical,modifiers,down}` | `ui.key` | `Key` press+release + `ModifiersChanged`（`:598-604`）[V] |
| `text {text}` | `ui.text` | `Event::Text`（`:605-608`）[V] |
| `drag {from,to,steps,button}` | `ui.drag` | `PointerMoved` + 按下 + 1..=200 步插值 + 释放（`:571-583`）[V] |
| `drop {paths}` | `ui.drop` | 进 `ModelApp::synthetic_drops`（`lib.rs:497`；`control.rs:609-624`）[V] |

键名用 `egui::Key::from_name`，并补 enter/return、esc/escape、delete、space、tab（`control.rs:76-87`）[V]；按钮 `"right"`→Secondary、`"middle"`→Middle、默认 Primary（`:68-73`）[V]。`raw_input_hook` 负责每帧只放一个指针事件/一段按键、补 `ModifiersChanged`、拖动保持按下时的修饰键（`lib.rs:2430-2449`）[V]——服务端不需要再写一份批处理。`focus {gained}` 映射到 RawInput 的 `focused`（本 app 自定义，不在 `control.rs` 的映射表里）[I]。

### 2.5 生命周期

1. 宿主设 `FIELDCRAFT_RENDER_TOKEN`、启动二进制（可选 `--read-only`/`--port`），读 `ready` 行拿端口。
2. `hello` ≤ 10 s，校验版本与 token；失败关连接。
3. 服务完成装配（设备、管线、首帧）后即可服务；设备初始化 ≤ 60 s（软件适配器更慢）[E]。
4. 稳态：`call`/`input`/`frame` 往返。
5. 停止：宿主 `stop` → 服务回 `bye {"why":"stop"}` 并 ≤5 s 退出；读线程 EOF → 立即退出（尽力先发 `bye "host-gone"`）。
6. 崩溃 = 无 `bye` 的退出或连接断开；宿主结束该代并在下次请求时重启。设备丢失：结束进程并回 `bye "device-lost"` [I]。

## 3. 构建与打包

### 3.1 workspace、xtask、许可证

- 新 app 由 `apps/*` glob 自动成为成员（`Cargo.toml:3`）[V]；`xtask/src/affected.rs` 的包发现基于 `cargo metadata`（`PACKAGE_ROOTS = ["crates/","apps/"]`，`:37,121-161`）[V]，无需硬编码。
- `xtask/src/layers.rs` 的 `TABLE` 必须加一行；`short_name` **只**剥 `fieldcraft-` 前缀（`layers.rs:126-128`）[V]，所以若包名为 `fieldz-render-server`，键必须写 **`("fieldz-render-server", Class::Exempt)`**，参照设计里的 `("render-server", …)` 在本 fork 不成立；若改名为 `fieldcraft-render-server`，键才是 `("render-server", …)` [V 机制；I 命名选择]。缺行会以 `Unregistered` 失败（`layers.rs:204-212`、`:161-167`），且 `cmd_ci` 的 quick gates 含 `cmd_layers`（`xtask/src/main.rs:232-255`）[V]。`Class::Exempt` 同时免除 wasm 构建要求（`main.rs:209-211`）[V]。
- 若要随版本发布：`xtask/src/licences.rs:15` 的 `ROOTS` 加 `"fieldz-render-server"`，否则其依赖的许可证不会进 `THIRD-PARTY-LICENSES.txt`（`:44-70`）[V]。仅内部/开发用则只需 TABLE 行 + `Cargo.lock` 变化。

### 3.2 mac / Windows / Linux 打包

三个打包脚本都**显式**按 `-p` 列表构建，不会自动发现新二进制：

- macOS：`packaging/macos/package.sh:70` 加 `-p fieldz-render-server`；`:75-80` 的 lipo、`:123-135` 的 `FieldCraft.app` 组装需要把新二进制放进 `Contents/MacOS`（若作为独立可执行文件分发，则另建 zip，参照 `:213-218` 的 CLI zip）[V]。
- Windows：`packaging/windows/package.ps1:47` 加 `-p fieldz-render-server`；`:60-62` 暂存 exe 与文档；`check-exe.ps1` 校验 PE 子系统（控制台为 3，GUI 为 2）与可选图标（`:5,16`）[V]。新 exe 默认无图标；需要图标就自带 `build.rs`（参照 `apps/fieldcraft/build.rs:1-20`）[V]。
- Linux：`packaging/linux/package.sh:53` 加 `-p fieldz-render-server`，并 `strip`；`:60-70` 的 FHS 树与 `install_share` 需要新增文件项；`.deb`/`.rpm` 的文件与依赖清单在 `apps/fieldcraft/Cargo.toml:78,110`，若随主包分发要同步 [V]。
- 文档：`packaging/env.sh:53` 的 `DOC_FILES` 含 `README.md LICENSE-MIT LICENSE-APACHE NOTICE THIRD-PARTY-LICENSES.txt`，`:56-61` 的 `copy_docs` 会复制进每个包；新包必须走到这条路径，四个文件缺一不可 [V]。
- 发布流水线硬编码产物名：`.github/workflows/release.yml:505-508` 的 publish 任务按名字收集产物，新增/改名要同步 [V]。
- `.cargo/config.toml:5-6` 让所有 workspace 二进制在 Windows 静态链接 CRT；`rust-toolchain.toml:6` 钉 1.99.1 [V]。

### 3.3 依赖与运行时

- 依赖集见 §1.2；**不引入 eframe/winit**，因此构建机不需要窗口开发库。`fieldcraft-gpu` 在非 wasm 下启用 `["metal","vulkan","dx12","gles"]`（`crates/gpu/Cargo.toml:18-21`）[V]；后端按平台在 `select.rs:92-100` 选 [V]。
- 无显示器的 Linux 要真正出图，需 `libvulkan1` + `mesa-vulkan-drivers`（lavapipe）；CI 正是这样装并以 `FIELDCRAFT_GPU=software FIELDCRAFT_REQUIRE_GPU=1` 跑无头 GPU（`.github/workflows/ci.yml:164-171`）[V]。`FIELDCRAFT_GPU` 的策略（auto/hardware/software/cpu）见 `select.rs:1-20,38-52,62-70` [V]。
- `[profile.release]` 的 opt-level 3 / thin LTO 自动适用于新包（`Cargo.toml:159-163`）[V]。

## 4. 首次构建的开放问题

第一版验收（写在构建日志里，不当作发布门槛）：能出整窗帧（面板 + 3D 视口）、能用 `input` 改一次模型并看到 `revision` 变化、能跑一个长任务并 `cancel` 成功、只读下 `file.save` 被拒且状态栏可见。

1. **无窗口 `RenderState`**：本 fork 钉的 egui-wgpu 0.37 git rev 源码不在盘上；“`RenderState::create(config, instance, None, options)` 仍存在且默认 `Rgba8Unorm`”只是从上游 0.36.2 推的 **[E]**。首要验证点；不成立就走 E5 的 CPU 兜底。
2. **整窗 egui-wgpu 离屏 pass 在软件适配器上的正确性**：仓库只证明了视口回调能画进离屏 pass（`gpu.rs:130-175`）[V]，没有整窗读回的先例 [I]。需在 lavapipe 上比对 CPU `capture()` 的一帧。
3. **CPU 兜底的成本**：`capture()` 每次两帧预热、CPU 栅格化 3D；能否支撑 ≥10 fps 未测 [E]。若不行，E5 的重构还要加“仅首帧/缩放后预热”的参数。
4. **包名与是否随版本发布**：`fieldz-render-server` 决定 layers 键、licences ROOTS、三个打包脚本与 `.deb` 清单；若内部专用，改动可减到 TABLE + lock。
5. **是否还需要搬 `apps/fieldcraft/src/control_server.rs`**：本设计判断不需要（传输不同，`control::handle` 已公开）[I]。若还有别的宿主只想用 JSON-lines 控制通道，则应另立 E7 做纯搬运。
6. **JPEG 依赖与画质/速度**：`image` 只是传递依赖，未在本仓库直接用过；q80、256 行分条、raw 阈值都要实测 [E]。若编码过慢，升级路径是 `turbojpeg`（首个真正的新依赖）[I]。
7. **同步命令不可抢占**：120 s 预算只能“返回后报告”，不能中断 UI 线程上的 `session.execute` [V `control.rs:256` 同步执行 / I 不可抢占]。宿主能否接受“迟到的 over_budget”需要产品确认。
8. **token 的下发**：环境变量还是 `ready` 行？环境变量更简单，但宿主进程表可能可见；`ready` 行要求首条消息前就信任 stdout [I]。
9. **变更通知缺失**：本 fork 没有 `control.subscribe`、`design.changed`、`document.revision`（`control.rs:697` 对未知方法回 `unknown method`）[V]。若宿主需要跟随用户编辑，只能轮询 `session.revision()`（定义 `crates/engine/src/lib.rs:755`，UI 侧用法见 `crates/ui-egui/src/lib.rs:743`）[V] 或后续新增一个小方法；第一版不做。
10. **wasm 编译面**：`crates/ui-egui/src/lib.rs:33` 无条件 `pub mod control;` [V]；本 app 是 native-only，不受影响；若将来把服务模块放进 `ui-egui`，必须加 `#[cfg(not(target_arch = "wasm32"))]`。