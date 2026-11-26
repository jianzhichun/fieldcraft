//! FieldCraft's own interface text in the chosen language.
//!
//! The message catalogue (`solidcraft_ui_text`, `locales/en.json` and `locales/zh.json`) holds the
//! ids the kit and the engine name. The strings FieldCraft writes itself — the app bar, the
//! browser, the start page, the forms, the results panel — are English literals at their call
//! sites, and [`tr`] and [`ts`] read them back in Chinese from the table below (the `chrome` half
//! of the translations). A string with no entry stays English. The language is the thread's
//! `solidcraft_ui_text` locale, so `solidcraft_ui_text::scope("zh")` switches both at once.
//!
//! `format!` templates go through [`wfmt!`]: it formats in English and splices the same values
//! into the Chinese template, so the placeholders, their order and their format specs all stay
//! the call site's.

use std::borrow::Cow;

/// English interface text → its Chinese translation, sorted by the English text (the lookup is a
/// binary search). Only strings FieldCraft draws itself; catalogue ids go through
/// `solidcraft_ui_text`.
pub static CHROME: &[(&str, &str)] = &[
    (" (auto)", "（自动）"),
    (" (capped)", "（上限）"),
    (" (from format {n})", "（来自格式 {n}）"),
    (" (thinnest wall {} {})", "（最薄壁 {} {}）"),
    (" (true scale)", "（真实比例）"),
    (" (virtual)", "（虚拟）"),
    (" (with results)", "（含结果）"),
    (" Deck", "输入文件"),
    (" at node {n}", "（节点 {n}）"),
    (" · proposed", " · 建议"),
    (" — Solve shows its results", " — 求解后显示其结果"),
    (" — not on this design: {}", " — 此设计上不可用：{}"),
    ("+ Add", "+ 添加"),
    (", bias {} {}", "，偏置 {} {}"),
    (", frame {k}", "，帧 {k}"),
    (", frame {k} of {n}: t = {}", "，帧 {k}/{n}：t = {}"),
    (", increment {k} of {}: load factor {}", "，增量 {k}/{}：载荷因子 {}"),
    (", mode {mode}: {value}", "，振型 {mode}：{value}"),
    (", smallest dihedral angle {d:.1}°", "，最小二面角 {d:.1}°"),
    (", smallest scaled Jacobian {}", "，最小缩放雅可比 {}"),
    (", {levels} octree levels{wall}", "，{levels} 层八叉树{wall}"),
    ("1 body", "1 个实体"),
    ("1 day ago", "1 天前"),
    ("A SolidCraft design, to mesh and set up", "SolidCraft 设计，用于划分网格和设置"),
    ("A command hit an internal error", "某条命令遇到内部错误"),
    (
        "A crash report was saved on this computer. Reporting it opens a GitHub issue you review first; nothing is sent until you submit it.",
        "崩溃报告已保存在本机。报告会先打开一个 GitHub 问题供你审阅；在你提交之前不会发送任何内容。",
    ),
    ("A different value along each axis", "每个轴使用不同的值"),
    ("A face-tagged 3MF surface mesh", "带面标签的 3MF 表面网格"),
    (
        "A linear static step has one state: plot a nonlinear static, explicit, heat or frequency step",
        "线性静力分析步只有一个状态：请绘制非线性静力、显式动力、导热或模态分析步",
    ),
    ("A model, a design or a mesh ({open_key})", "模型、设计或网格（{open_key}）"),
    (
        "A new model starts empty with this design; keeping the model keeps its mesh and definitions, which then refer to bodies the design does not have.",
        "新模型从此设计开始且为空；保留模型则保留其网格和定义，它们随后会引用设计中不存在的实体。",
    ),
    ("A new version of FieldZ is available", "有新的 FieldZ 版本可用"),
    ("A solve stopped", "求解已停止"),
    ("A value of another shape (from a script); Reset starts the field again", "形状不符的值（来自脚本）；重置可重新开始填写该字段"),
    ("Active", "激活"),
    ("Active in step", "在分析步中活动"),
    ("Add", "添加"),
    ("Add boundary condition…", "添加边界条件…"),
    ("Add load…", "添加载荷…"),
    ("Add material…", "添加材料…"),
    ("Add point", "添加点"),
    ("Add row", "添加行"),
    ("Add section…", "添加截面…"),
    ("Add set…", "添加集合…"),
    ("Add step…", "添加分析步…"),
    ("Added", "已添加"),
    ("Alias", "Alias"),
    ("All", "全部"),
    ("All nodes", "所有节点"),
    ("An Abaqus, Nastran or LS-DYNA input deck", "Abaqus、Nastran 或 LS-DYNA 输入文件"),
    ("And {} more since.", "此后还有 {} 项。"),
    ("Animate", "动画"),
    (
        "Animate shape and contours. Linear static results interpolate a proportional load; nonlinear steps play their converged increments with their own stresses, explicit steps play saved fields.",
        "对形状和云图做动画。线性静力结果按比例载荷插值；非线性分析步按各自的收敛增量及其应力播放，显式动力步播放已保存的场。",
    ),
    ("Animating", "动画播放中"),
    ("Animation stopped", "动画已停止"),
    ("Applied F", "施加的 F"),
    ("Applied force", "施加力"),
    ("Apply this {noun} in step {step}, or leave it out of that step", "在分析步 {step} 中应用此{noun}，或将其排除在该步之外"),
    ("Apply this {noun} to the mesh, in its form", "在其表单中将此{noun}应用到网格"),
    ("Apply…", "应用…"),
    ("Area", "面积"),
    ("Ask GitHub Releases whether a newer FieldZ is published", "向 GitHub Releases 查询是否已发布更新的 FieldZ"),
    ("At t = {}{frame}", "在 t = {}{frame}"),
    ("Attach the commands that built the model, to replay it. They can name your files.", "附加构建模型的命令以便重放。其中可能包含你的文件名。"),
    ("Attach the model too (you attach the file yourself).", "同时附加模型（文件由你自行附加）。"),
    ("Attach {} ({}): {}", "附加 {}（{}）：{}"),
    ("Auto", "自动"),
    ("Autosaved {} · {} nodes, {} elements", "已自动保存 {} · {} 个节点，{} 个单元"),
    ("Back to the model", "返回模型"),
    ("Back to {name}", "返回 {name}"),
    ("Balance", "平衡"),
    ("Boundary Condition", "边界条件"),
    ("Box", "框选"),
    ("Box selection picks nodes, element faces and elements", "框选可选取节点、单元面和单元"),
    (
        "Box selection: a left drag selects the visible items inside the box (right or middle drag pans)",
        "框选：左键拖动选择框内可见项（右键或中键拖动平移）",
    ),
    ("Bring this {noun} back into the model", "将此{noun}重新纳入模型"),
    ("Browser", "模型树"),
    ("Buckling load factor", "屈曲载荷因子"),
    ("Cancel", "取消"),
    ("Centre this field extreme", "将此场极值居中"),
    ("Change this {noun} in its form", "在其表单中修改此{noun}"),
    ("Change this {noun} in its form: saving replaces it", "在其表单中修改此{noun}：保存将替换它"),
    ("Changed", "已更改"),
    ("Check for Updates", "检查更新"),
    ("Choose…", "选择…"),
    ("Clear it: the field takes its default", "清除：字段恢复默认值"),
    ("Clear selection", "清除选择"),
    ("Clear the selection (Esc)", "清除选择（Esc）"),
    ("Clear this seed (undo brings it back)", "清除此种子（撤销可恢复）"),
    ("Click the model to pick the node", "单击模型以选取节点"),
    ("Click the model to probe a node", "单击模型以探点节点"),
    (
        "Click the visible mesh to add reference-coordinate points, including on deformed and curved element faces.",
        "点击可见网格以添加参考坐标点，包括变形和弯曲的单元面。",
    ),
    ("Close", "关闭"),
    ("Close the model", "关闭模型"),
    ("Close the model: a new, empty one takes its place (File › New)", "关闭模型：将出现一个新的空模型（文件 › 新建）"),
    ("Cmd", "Cmd"),
    ("Collapse the browser", "折叠模型树"),
    ("Colour the part by the selected fluid field or wall heat", "按所选流体场或壁面热流为零件着色"),
    ("Coloured", "已着色"),
    ("Completed", "已完成"),
    ("Component temperatures", "分量温度"),
    ("Contact", "接触"),
    ("Contours", "云图"),
    ("Contours off on {step}", "{step} 上云图关闭"),
    ("Copied the crash report's folder", "已复制崩溃报告所在文件夹"),
    ("Copy Path", "复制路径"),
    ("Copy the value", "复制数值"),
    ("Coupling", "耦合"),
    ("Crash Reports", "崩溃报告"),
    ("Crash Reports…", "崩溃报告…"),
    ("Crash reports: {e}", "崩溃报告：{e}"),
    ("Create set from selection…", "从选择创建集合…"),
    ("Create {}…", "创建 {}…"),
    ("Ctrl", "Ctrl"),
    ("Dark theme", "深色主题"),
    ("Datum", "基准"),
    ("Deactivate in step {one}", "在分析步 {one} 中停用"),
    ("Deactivate in {} steps", "在 {} 个分析步中停用"),
    ("Deactivating it there lets it be deleted next.", "在那里停用后即可随后删除。"),
    ("Defeature", "移除特征"),
    ("Defeature the {} proposed", "对建议的 {} 去特征"),
    ("Deformation × {}{how}", "变形 × {}{how}"),
    ("Deformed", "变形"),
    ("Deformed shape", "变形形状"),
    ("Degrees of freedom", "自由度"),
    ("Delete", "删除"),
    ("Delete All", "全部删除"),
    ("Delete this {noun} (undo brings it back)", "删除此{noun}（撤销可恢复）"),
    ("Delete this {noun}: asks first", "删除此{noun}：会先询问"),
    ("Delete this {noun}: asks first, listing what uses it (Delete key)", "删除此{noun}：会先询问并列出使用它的项（Delete 键）"),
    ("Delete this {} (Delete key)", "删除此 {}（Delete 键）"),
    ("Delete {what} and what uses it: {}?", "删除 {what} 及其使用的内容：{}？"),
    ("Delete {what}?", "删除 {what}？"),
    ("Deleted", "已删除"),
    ("Deleted {n} crash report{}", "已删除 {n} 份崩溃报告{}"),
    ("Delete…", "删除…"),
    ("Details", "详细信息"),
    ("Did not open {}", "未打开 {}"),
    ("Discard", "丢弃"),
    ("Discard All", "全部放弃"),
    ("Display", "显示"),
    ("Display: what the view shows, the projection, the panels and the mouse", "显示：视图显示的内容、投影方式、面板和鼠标"),
    ("Don't Save", "不保存"),
    ("Done", "完成"),
    ("Download this version again, verify it and swap it in, to repair a damaged install", "重新下载此版本、校验并替换，以修复损坏的安装"),
    (
        "Draft a GitHub issue with the version, the OS and the last error; nothing is sent until you submit it",
        "起草一个包含版本、操作系统和最近错误的 GitHub issue；在你提交之前不会发送任何内容",
    ),
    ("Draw the deformed shape", "绘制变形形状"),
    ("Draw the displacements at true scale (they are large already)", "按真实比例绘制位移（其值已经很大）"),
    (
        "Drop to open: .fieldcraft models, .solidcraft designs, .3mf tagged meshes, .msh orphan meshes",
        "拖放以打开：.fieldcraft 模型、.solidcraft 设计、.3mf 标记网格、.msh 孤立网格",
    ),
    ("Edge ratio", "边长比"),
    ("Edges", "边"),
    ("Edit", "编辑"),
    ("Edit…", "编辑…"),
    ("Elements", "单元"),
    ("Empty: add geometry, a mesh and the setup", "空：添加几何、网格和设置"),
    ("End time", "结束时间"),
    ("Energies", "能量"),
    ("Energy balance error", "能量平衡误差"),
    ("Energy: {}", "能量：{}"),
    ("Enter", "Enter"),
    ("Error: blocks solving", "错误：阻止求解"),
    ("Esc", "Esc 键"),
    ("Every command that changes the model appears here", "每条修改模型的命令都会出现在这里"),
    ("Every component stays under its limit ({limited} checked)", "每个分量都低于其限值（已检查 {limited} 个）"),
    ("Every model", "所有模型"),
    ("Expand the browser", "展开模型树"),
    ("Export CSV…", "导出 CSV…"),
    ("External work", "外力功"),
    ("Face Merge", "面合并"),
    ("Face Split", "面分割"),
    ("Face-tagged mesh (3MF)", "面标记网格（3MF）"),
    ("Faces", "面"),
    ("Faces, edges", "面、边"),
    ("Feature", "特征"),
    ("Field", "场"),
    ("Field not saved at this increment", "此增量处未保存场"),
    ("Field unavailable", "场不可用"),
    ("FieldZ closed unexpectedly", "FieldZ 意外关闭"),
    ("FieldZ closed unexpectedly the last time it ran.", "FieldZ 上次运行时意外关闭。"),
    ("FieldZ hit an internal error", "FieldZ 遇到内部错误"),
    ("FieldZ hit an internal error the last time it ran.", "FieldZ 上次运行时遇到内部错误。"),
    ("FieldZ model", "FieldZ 模型"),
    ("FieldZ script (JSON)", "FieldZ 脚本（JSON）"),
    ("FieldZ {version} is available", "FieldZ {version} 可用"),
    ("FieldZ {}", "FieldZ {}"),
    ("File", "文件"),
    ("Final state", "最终状态"),
    ("Finish or cancel the open file chooser first", "请先完成或取消已打开的文件选择器"),
    ("Fit", "适配"),
    ("Fit the model (or a double click)", "适配模型（或双击）"),
    ("Flow", "流动"),
    ("Fluid Domain", "流体域"),
    ("Fluid Domains", "流体域"),
    ("Follow the field's {}; untick to clamp the legend", "跟随场的 {}；取消勾选可固定图例"),
    ("Force–travel {upper}: {set}", "力–行程 {upper}：{set}"),
    ("Frame", "帧"),
    ("Frames", "帧"),
    ("Frequency", "模态"),
    ("Frequency [Hz]", "模态频率 [Hz]"),
    ("From the displayed frame", "来自显示的帧"),
    ("Fusion", "Fusion"),
    ("Gap / undefined", "间断 / 未定义"),
    ("Generate mesh…", "生成网格…"),
    ("Geometry", "几何"),
    ("Heat flow into the body", "流入实体的热流"),
    ("Heat flow through", "穿过的热流"),
    ("Help: navigation, keys, updates, about", "帮助：导航、快捷键、更新、关于"),
    ("Hidden for wall heat", "壁面热流时隐藏"),
    ("Hide", "隐藏"),
    ("Hide this {noun} in the viewport", "在视口中隐藏此{noun}"),
    ("Highest over the frames, at t = {at}", "各帧最高，在 t = {at}"),
    ("History", "历史"),
    ("History node {node}: {}", "历史节点 {node}：{}"),
    ("Home view", "主视图"),
    ("Hourglass", "沙漏能"),
    ("Imbalance", "不平衡量"),
    ("Import mesh…", "导入网格…"),
    ("Imported", "已导入"),
    ("Importing", "导入中"),
    ("In", "流入"),
    ("In mesh", "网格中"),
    ("Include the model file", "包含模型文件"),
    ("Increment", "增量"),
    ("Initial Field", "初始场"),
    ("Inlets and outlets", "入口和出口"),
    ("Interactions", "相互作用"),
    ("Internal", "内能"),
    ("Invalid", "无效"),
    ("Inventor", "Inventor"),
    ("Invert", "反选"),
    ("Isolate", "隔离"),
    ("It needs:", "它需要："),
    ("Its step no longer has this rigid wall", "其分析步已不再有此刚性墙"),
    ("Jacobian ratio", "雅可比比率"),
    ("Keep the Current Model", "保留当前模型"),
    ("Keep the camera and this results display as a named view, saved with the model", "将相机和此结果显示保存为命名视图，随模型一同保存"),
    ("Keep this {noun}, but leave it out of the model", "保留此{noun}，但不纳入模型"),
    ("Kind", "类型"),
    ("Kinetic", "动能"),
    ("Labels", "标签"),
    (
        "Left drag does the navigation bar's mode (orbit, pan or zoom); right, middle or Shift+left drag pans",
        "左键拖动执行导航栏的模式（旋转、平移或缩放）；右键、中键或 Shift+左键拖动平移",
    ),
    (
        "Left drag orbits (or pans or zooms: the navigation bar), right or middle drag pans, the wheel zooms, F or a double click fits.",
        "左键拖动旋转（或平移或缩放：视导航栏而定），右键或中键拖动平移，滚轮缩放，F 或双击适配。",
    ),
    (
        "Left drag orbits; right or middle drag pans, Shift+right or Shift+middle drag zooms",
        "左键拖动旋转；右键或中键拖动平移，Shift+右键或 Shift+中键拖动缩放",
    ),
    ("Legend clamped to {} … {}", "图例固定为 {} … {}"),
    ("Legend max", "图例最大值"),
    ("Legend min", "图例最小值"),
    ("Light theme", "浅色主题"),
    ("Limit", "限值"),
    ("Load", "载荷"),
    ("Load factor", "载荷因子"),
    ("Loads & BCs", "载荷和边界条件"),
    ("Make a set whose members are the selection", "创建以当前选择为成员的集合"),
    ("Margin", "裕度"),
    ("Mark the nodes of the largest and smallest value", "标记最大值和最小值所在的节点"),
    ("Mass flow", "质量流量"),
    (
        "Mass uses reference density. Heat is advective transport, relative to the mixed inflow temperature below; wall heat, conduction and storage are separate.",
        "质量采用参考密度。热量为对流传热，相对于下方的混合流入温度；壁面热流、导热和储能另行计算。",
    ),
    ("Material", "材料"),
    ("Materials", "材料"),
    ("Max", "最大"),
    ("Max Mises", "最大 Mises"),
    ("Max T", "最高 T"),
    ("Max plastic strain", "最大塑性应变"),
    ("Max |U|", "最大 |U|"),
    ("Mesh", "网格"),
    ("Mesh Plan", "网格方案"),
    ("Mesh…", "网格…"),
    ("Middle drag orbits, Ctrl+middle pans, Shift+middle zooms, right drag orbits", "中键拖动旋转，Ctrl+中键平移，Shift+中键缩放，右键拖动旋转"),
    ("Middle drag pans, Shift+middle orbits, Ctrl+middle zooms, right drag orbits", "中键拖动平移，Shift+中键旋转，Ctrl+中键缩放，右键拖动旋转"),
    ("Middle drag pans, Shift+middle orbits, right drag orbits", "中键拖动平移，Shift+中键旋转，右键拖动旋转"),
    ("Min", "最小"),
    ("Min T", "最低 T"),
    ("Mixed inflow temperature", "混合流入温度"),
    ("Mixed temperature", "混合温度"),
    ("Mode", "模式"),
    ("Mode amplitude", "振型幅值"),
    ("Mode shape (normalised)", "振型（归一化）"),
    ("Mode {mode}: {} Hz", "振型 {mode}：{} Hz"),
    ("Model", "模型"),
    (
        "Model data only. Analysis history was not imported; review unread keywords and warnings before solving.",
        "仅模型数据。分析历史未导入；求解前请检查未读取的关键字和警告。",
    ),
    ("Models autosaved by a FieldZ that crashed or closed with unsaved changes.", "由崩溃或关闭时仍有未保存更改的 FieldZ 自动保存的模型。"),
    ("Models autosaved by an app that crashed or closed with unsaved changes", "由崩溃或未保存更改即关闭的应用自动保存的模型"),
    ("Models, designs and meshes", "模型、设计和网格"),
    ("Models, designs and meshes you open or save are listed here.", "你打开或保存的模型、设计和网格会列在这里。"),
    ("Modes", "振型"),
    ("Moments about ({})", "绕 ({}) 的力矩"),
    (
        "Names a set of the selection here: an existing set with the same members, or a new one made on OK",
        "为此处的所选内容命名一个集合：成员相同的现有集合，或在点击确定时新建的集合",
    ),
    ("Navigation", "导航"),
    ("Net heat out", "净热流出"),
    ("Net mass out", "净质量流出"),
    ("New", "新建"),
    ("New model", "新建模型"),
    ("New model (File › New)", "新建模型（文件 › 新建）"),
    ("Next: {} — {}", "下一步：{} — {}"),
    ("No analysis steps yet: Steps › Step creates one.", "尚无分析步：分析步 › 分析步可创建一个。"),
    ("No autosaved models to recover.", "没有可恢复的自动保存模型。"),
    ("No crash reports are saved on this computer.", "本机没有保存的崩溃报告。"),
    ("No defined values along this path", "此路径上没有已定义的值"),
    ("No stress output, or past the step's field cap", "无应力输出，或超出该分析步的场上限"),
    ("No temperature limits: set one per conducting solid", "无温度限值：请为每个导热实体设置一个"),
    ("No wall heat recorded at this frame", "此帧未记录壁面热流"),
    ("No {} yet", "尚无{}"),
    ("Node {}", "节点 {}"),
    ("Nodes", "节点"),
    ("None", "无"),
    ("None saved", "未保存任何项"),
    ("Normal {}", "法向 {}"),
    ("Not Now", "暂不"),
    ("Not active in this step", "此分析步中未激活"),
    ("Not editable: delete it and make it again", "不可编辑：删除它并重新创建"),
    ("Not made by a mesher in this model (imported or built by hand)", "并非本模型中的网格生成器所生成（导入或手工构建）"),
    ("Not measured: {}", "未测量：{}"),
    ("Not saved yet", "尚未保存"),
    ("Nothing to redo", "没有可重做的操作"),
    ("Nothing to undo", "没有可撤销的操作"),
    ("Nothing to undo: every command shown is {BEYOND_HISTORY}", "无可撤销的操作：显示的所有命令均 {BEYOND_HISTORY}"),
    ("Obstacles", "障碍物"),
    ("Obstacles coloured by the flow", "障碍物按流动着色"),
    ("Obstacles in their own colours", "障碍物使用自身颜色"),
    ("Open GitHub Issue", "打开 GitHub 问题"),
    (
        "Open Relax Nodes with its sphere round the element of smallest scaled Jacobian; preview, then OK",
        "打开 Relax Nodes 并将球置于缩放雅可比最小的单元周围；预览，然后确定",
    ),
    (
        "Open a model, a SolidCraft design or a tagged mesh, or drop one here. The samples are ready to solve.",
        "打开模型、SolidCraft 设计或带标签的网格，或将其中一个拖放到此处。样例可直接求解。",
    ),
    ("Open design", "打开设计"),
    ("Open design…", "打开设计…"),
    ("Opened", "已打开"),
    ("Opened the {l} sample", "已打开 {l} 示例"),
    ("Opened the {} sample", "已打开 {} 示例"),
    ("Opened {name} as a new model", "已将 {name} 作为新模型打开"),
    ("Opening", "正在打开"),
    ("Opening the {} sample", "正在打开 {} 示例"),
    ("Opening {name}", "正在打开 {name}"),
    ("Open…", "打开…"),
    ("Orbit", "旋转"),
    ("Orbit: left drag turns the view", "旋转：左键拖动转动视图"),
    ("Orphan mesh", "孤立网格"),
    ("Orphan mesh (Gmsh MSH 4.1)", "孤立网格（Gmsh MSH 4.1）"),
    ("Orthographic", "正交"),
    ("Out", "流出"),
    ("Pan", "平移"),
    ("Pan: left drag moves the view (or right or middle drag)", "平移：左键拖动移动视图（或右键、中键拖动）"),
    ("Parameters", "参数"),
    ("Partition", "分割"),
    ("Path Plot", "路径图"),
    ("Path already has 1000 points", "路径已有 1000 个点"),
    ("Path plots currently use solid nodal fields", "路径图目前使用实体节点场"),
    ("Peak", "峰值"),
    ("Per axis", "按轴"),
    ("Perspective", "透视"),
    ("Pick", "拾取"),
    ("Pick in the viewport, or choose below", "在视口中拾取，或在下方选择"),
    ("Pick it in the viewport", "在视图中选取它"),
    ("Pick its face in the viewport", "在视图中选取它的面"),
    ("Pick surface points", "拾取表面点"),
    ("Pick the source face and use it, then pick the target surface and use it", "拾取源面并使用，然后拾取目标面并使用"),
    ("Pick this body in the viewport", "在视图中选取此实体"),
    ("Play the frames from the start-up transient to the settled flow", "播放从启动瞬态到稳定流动的各帧"),
    ("Plotted result is stale; sample again after solving", "绘图结果已过期；求解后请重新采样"),
    ("Plotted {what}: {}", "已绘制 {what}：{}"),
    ("Point Mass", "点质量"),
    ("Point {}", "点 {}"),
    ("Port fields unavailable", "端口场不可用"),
    ("Position", "位置"),
    ("Positive into wall; face average", "正值流入壁面；按面平均"),
    ("Prescribed temperatures", "给定温度"),
    ("Preset", "预设"),
    ("Projection", "投影"),
    ("Proportional load", "比例载荷"),
    (
        "Puts the point at what is selected: a vertex or node, the centre of a hole (its faces or rim), or the centroid of faces and edges",
        "将点放在所选内容处：顶点或节点、孔的中心（其面或边缘），或面和边的形心",
    ),
    (
        "Puts the point on what is selected: a vertex, an edge's midpoint, a face's centre, or the centre of a hole or of several items",
        "将点放在所选内容上：顶点、边的中点、面的中心，或孔或若干项的中心",
    ),
    ("Quantity", "量"),
    ("Reaction force", "反力"),
    ("Reaction force and moment per boundary-condition set", "每个边界条件集合的反力和力矩"),
    ("Reaction sums", "反力合计"),
    ("Reaction {upper} on {set}", "{set} 上的反力 {upper}"),
    ("Reactions and applied loads differ by {}", "反力与施加载荷相差 {}"),
    ("Reactions balance the applied loads", "反力与施加载荷平衡"),
    ("Read Abaqus Deck: {} unread keywords (details in the panel)", "读取 Abaqus 输入文件：{} 个未读取关键字（详情见面板）"),
    ("Read Deck…", "读取输入文件…"),
    ("Read deck…", "读取输入文件…"),
    ("Recent", "最近"),
    ("Recover", "恢复"),
    ("Recovered the autosaved model", "已恢复自动保存的模型"),
    ("Recovering the autosaved model", "正在恢复自动保存的模型"),
    ("Recover…", "恢复…"),
    ("Redid {}", "已重做 {}"),
    ("Redo ({})", "重做（{}）"),
    ("Reference Point", "参考点"),
    ("Reference coordinates [{length}]: x, y, z", "参考坐标 [{length}]：x, y, z"),
    ("Reinstall FieldZ…", "重新安装 FieldZ…"),
    ("Relax around worst element", "围绕最差单元松弛"),
    ("Relaxation Field", "松弛场"),
    ("Reload to update", "重新加载以更新"),
    ("Remove", "移除"),
    ("Remove point", "移除点"),
    ("Remove this row", "移除此行"),
    ("Rename this {noun}: every definition that refers to it follows", "重命名此{noun}：所有引用它的定义随之更新"),
    ("Rename…", "重命名…"),
    ("Render", "渲染"),
    ("Replace", "替换"),
    ("Replace mesh", "替换网格"),
    ("Report It…", "报告…"),
    ("Report a Bug", "报告问题"),
    ("Report a Bug: could not open the browser: {e}", "报告问题：无法打开浏览器：{e}"),
    ("Report a Bug: could not save the files to attach: {e}", "报告问题：无法保存待附加的文件：{e}"),
    ("Report a Bug…", "报告错误…"),
    ("Report a bug", "报告错误"),
    (
        "Report a bug: draft a GitHub issue with the version, the OS and the last error; nothing is sent until you submit it",
        "报告错误：起草一个包含版本、操作系统和最近错误的 GitHub issue；在你提交之前不会发送任何内容",
    ),
    ("Report…", "报告…"),
    ("Required", "必填"),
    ("Reset", "重置"),
    ("Restore", "还原"),
    ("Result is stale; solve it again", "结果已过期；请重新求解"),
    ("Results", "结果"),
    ("Reverse zoom", "反向缩放"),
    ("Right drag orbits, middle or Shift+right drag pans", "右键拖动旋转，中键或 Shift+右键拖动平移"),
    ("Rigid wall on {} ({})", "{} 上的刚性墙（{}）"),
    ("Run Script needs a file chooser; scripts also run with fieldcraft-cli run", "运行脚本需要文件选择器；脚本也可通过 fieldcraft-cli run 运行"),
    ("Run Script…", "运行脚本…"),
    ("Run again", "再次运行"),
    ("Run {}", "运行 {}"),
    ("Runs the command with these parameters", "使用这些参数运行命令"),
    ("S or {} searches every command; `id {{json}}` typed there runs it.", "S 或 {} 搜索所有命令；在其中输入 `id {{json}}` 即可运行它。"),
    ("Sample path", "采样路径"),
    ("Samples", "样例"),
    ("Samples…", "示例…"),
    ("Save", "保存"),
    ("Save ({})", "保存（{}）"),
    (
        "Save As, with a copy of the design and imported meshes inside the file, so it opens anywhere: whoever you send it to gets your geometry too",
        "另存为，文件内含设计副本和导入的网格，可在任何地方打开：接收者也会得到你的几何",
    ),
    ("Save As…", "另存为…"),
    ("Save View…", "保存视图…"),
    ("Save With Geometry Embedded…", "内嵌几何保存…"),
    ("Save the changes to {} before opening {}?", "打开 {} 之前保存对 {} 的更改？"),
    ("Save the changes to {} before restarting to update?", "重启以更新之前保存对 {} 的更改？"),
    (
        "Saved on this computer, newest first. Reporting one opens a GitHub issue you review first; nothing is sent until you submit it.",
        "保存在本机，最新的在前。报告会先打开一个 GitHub 问题供你审阅；在你提交之前不会发送任何内容。",
    ),
    ("Saved view", "已保存视图"),
    ("Saved {}{}", "已保存 {}{}"),
    ("Scale", "比例"),
    ("Scale the largest displacement to a tenth of the model", "将最大位移缩放到模型的十分之一"),
    ("Scaled Jacobian", "缩放雅可比"),
    ("Section", "截面"),
    ("Sections", "截面"),
    ("Section…", "截面…"),
    ("Seed", "种子"),
    ("Seed edges…", "布置边种子…"),
    ("Seeds", "种子"),
    ("Seeds on the inlet face, upstream of the obstacles", "障碍物上游入口面上的种子"),
    ("Select", "选择"),
    ("Select and frame it in the viewport", "选择并在视图中框定它"),
    ("Select and frame them in the viewport", "选择并在视图中框定它们"),
    ("Select the faces it made; show its details", "选择它生成的面；显示其详细信息"),
    ("Select the {} {what} elements", "选择 {} 个 {what} 单元"),
    ("Select {subject} to see and edit it", "选择 {subject} 以查看和编辑"),
    ("Select {}", "选择 {}"),
    ("Selection", "选择"),
    ("Sensible heat", "显热"),
    ("Set", "集合"),
    ("Sets", "集合"),
    ("Settings: units, the window, privacy and updates", "设置：单位、窗口、隐私和更新"),
    ("Settings…", "设置…"),
    ("Shape", "形状"),
    ("Share my session", "共享我的会话"),
    (
        "Shift+Alt+left drag orbits, Shift+Alt+middle pans, Shift+Alt+right zooms; middle drag also pans and right drag orbits",
        "Shift+Alt+左键拖动旋转，Shift+Alt+中键平移，Shift+Alt+右键缩放；中键拖动也可平移，右键拖动旋转",
    ),
    ("Show", "显示"),
    ("Show All", "显示全部"),
    ("Show and probe node {node}", "显示并探点节点 {node}"),
    ("Show everything that is hidden", "显示所有隐藏的内容"),
    ("Show its details", "显示其详细信息"),
    ("Show only this {noun}", "仅显示此{noun}"),
    ("Show results…", "显示结果…"),
    ("Show the model as this view saved it", "按此视图保存时的样子显示模型"),
    ("Show this step before picking its surface", "拾取其表面前先显示该分析步"),
    ("Show this view: its camera and results display", "显示此视图：其相机和结果显示"),
    ("Show this {noun} in the viewport", "在视口中显示此{noun}"),
    ("Showing mode {mode} of {step}", "显示 {step} 的振型 {mode}"),
    ("Showing {field} on {step}", "在 {step} 上显示 {field}"),
    ("Showing {step} deformed{scale}", "显示 {step} 变形{scale}"),
    ("Showing {step} frame {f}", "显示 {step} 帧 {f}"),
    ("Showing {step} undeformed", "显示 {step} 未变形"),
    ("Showing {step}, last frame", "显示 {step}，最后一帧"),
    ("Size", "尺寸"),
    ("Size Control", "尺寸控制"),
    ("Slice", "切片"),
    ("Slice off on {step}", "{step} 上切片关闭"),
    ("SolidCraft design", "SolidCraft 设计"),
    ("Solve", "求解"),
    ("Solve cancelled", "求解已取消"),
    ("Solve every step and show the results", "求解每个分析步并显示结果"),
    ("Solve the step again before plotting a path", "绘制路径前请重新求解该分析步"),
    ("Solve: not ready yet (or try Home › Samples)", "求解：尚未就绪（或试试主页 › 样例）"),
    ("Solved {n} step{}", "已求解 {n} 个分析步{}"),
    ("Solved {steps} step{}", "已求解 {steps} 个分析步{}"),
    ("Solved {step}", "已求解 {step}"),
    ("Solved {}", "已求解 {}"),
    ("Solver deck", "求解器输入文件"),
    ("Solve…", "求解…"),
    ("Solving", "求解中"),
    ("Spaces become _ (names go into solver decks).", "空格会变为 _（名称将写入求解器输入文件）。"),
    ("Spring or Dashpot", "弹簧或阻尼器"),
    ("Stale: the model changed since this step was solved", "已过期：该分析步求解后模型已更改"),
    ("Start", "开始"),
    ("Start a New Model", "新建模型"),
    ("Start: new, open, recent files and samples", "开始：新建、打开、最近文件和样例"),
    ("Static pressure", "静压"),
    ("Steady value (t = {at})", "稳定值（t = {at}）"),
    ("Step", "分析步"),
    ("Step {}", "分析步 {}"),
    ("Steps", "分析步"),
    ("Step…", "分析步…"),
    ("Stitch", "缝合"),
    ("Stored frame (otherwise final)", "已存帧（否则为最终状态）"),
    ("Streamlines", "流线"),
    ("Streamlines from the inlet, coloured by speed or temperature when selected", "从入口发出的流线，选中时按速度或温度着色"),
    ("Streamlines off", "流线已关闭"),
    ("Suppress", "抑制"),
    ("Switch off: defeature {name} takes it out", "关闭：去特征 {name} 会将其移除"),
    ("Switch on: defeature {by} keeps it", "开启：去特征 {by} 会保留它"),
    ("Symbol size", "符号大小"),
    ("Symbols sampled: {}", "采样符号：{}"),
    ("Tagged mesh", "带标签的网格"),
    ("Take them out with defeature {name}", "用去特征 {name} 将其移除"),
    ("Target", "目标"),
    ("That command is {BEYOND_HISTORY}", "该命令 {BEYOND_HISTORY}"),
    ("The crash reports saved on this computer: report, copy or delete them", "本机保存的崩溃报告：报告、复制或删除它们"),
    ("The element faces picked in the viewport (Select: Element faces)", "在视口中拾取的单元面（选择：单元面）"),
    ("The energy balance's relative error", "能量平衡的相对误差"),
    ("The file chooser could not open; try again", "文件选择器无法打开；请重试"),
    ("The flow", "流动"),
    (
        "The issue form opened in your browser: attach the files in {} and submit it there.",
        "问题表单已在浏览器中打开：请附加 {} 中的文件并在那里提交。",
    ),
    ("The issue form opened in your browser: review it and submit it there.", "问题表单已在浏览器中打开：请审阅并在那里提交。"),
    ("The model changed while the file chooser was open; choose the file again", "文件选择器打开期间模型已更改；请重新选择文件"),
    ("The model is here: drag to roll it back or forward", "模型在此处：拖动可回滚或前进"),
    ("The model is ready to solve.", "模型已可求解。"),
    ("The one {} selected in the view", "视图中选中的那个{}"),
    ("The scale the displacements are drawn at", "位移绘制所用的比例"),
    ("The seeds the grid across the inlet face fits", "入口面上网格可容纳的种子数"),
    ("The seeds the streamlines were given", "流线所用的种子"),
    (
        "The selected bodies (Select: Bodies), or the bodies of the selected faces, become the cells",
        "选中的几何体（选择：几何体），或所选面的几何体，成为区域",
    ),
    ("The selected design edges (Select: Edges) become the edges seeded", "选中的设计边（选择：边）成为布置种子的边"),
    (
        "The selected design faces (Shift-click to pick, Select: Faces) become the form's faces, in place of a sphere",
        "选中的设计面（Shift+点击拾取，选择：面）取代球面成为该形状的面",
    ),
    ("The selection becomes the set's members (geometry by name, the mesh by label)", "所选内容成为该集合的成员（几何按名称，网格按标签）"),
    ("The session goes on the clipboard too, to paste.", "会话也会复制到剪贴板以便粘贴。"),
    ("The size of boundary condition, load and interaction symbols on screen, at any zoom", "屏幕上边界条件、载荷和相互作用符号的大小，与缩放无关"),
    ("The symbols of the step's boundary conditions and loads", "该分析步边界条件和载荷的符号"),
    ("The wheel zooms the other way", "滚轮以相反方向缩放"),
    (
        "The {commands} command(s) that built the model, the version and the OS; they can name your files.",
        "构建模型的 {commands} 条命令、版本和操作系统；其中可能包含你的文件名。",
    ),
    ("The {}s picked in the viewport (Select: {}s)", "在视口中拾取的 {}s（选择：{}s）"),
    ("There is no sample `{name}`: choose one of these", "没有示例 `{name}`：请从以下选择一个"),
    ("These parameters do not fit this form: cancel it and start the command again", "这些参数不适用于此表单：请取消并重新启动该命令"),
    ("They are deleted too; Undo brings everything back.", "它们也会被删除；撤销可恢复全部内容。"),
    (
        "This opens a GitHub issue in your browser, filled in with what is shown below. Nothing is sent until you submit it there.",
        "这会在浏览器中打开一个 GitHub 问题，并填入下方所示内容。在你于那里提交之前不会发送任何内容。",
    ),
    ("Tick the unticked ones and untick the others", "勾选未勾选项，取消勾选其余项"),
    ("Tie", "绑定"),
    ("Time reached", "达到时间"),
    ("Timeline", "时间线"),
    ("Tinkercad", "Tinkercad"),
    ("Title", "标题"),
    ("Title: {}", "标题：{}"),
    ("Total mass", "总质量"),
    ("Traced {}", "已追踪 {}"),
    ("Type", "类型"),
    ("Undeformed", "未变形"),
    ("Undetermined", "未确定"),
    ("Undid {}", "已撤销 {}"),
    ("Undo ({})", "撤销（{}）"),
    ("Units, the window, privacy and updates", "单位、窗口、隐私和更新"),
    ("Units…", "单位…"),
    ("Unsaved changes", "未保存的更改"),
    ("Unsuppress", "取消抑制"),
    ("Untitled", "未命名"),
    ("Update", "更新"),
    ("Update to {version}", "更新到 {version}"),
    ("Use as source", "用作源"),
    ("Use as target surface", "用作目标面"),
    ("Use selection", "使用所选内容"),
    ("Use selection ({} face{})", "使用选择（{} 个面{}）"),
    ("Use selection ({} {}{})", "使用选择（{} {}{}）"),
    ("Valid", "有效"),
    ("View {i}", "视图 {i}"),
    ("Viewport: {e}", "视图：{e}"),
    ("Views", "视图"),
    ("Visual style", "视觉样式"),
    ("Volume", "体积"),
    ("Volume flow", "体积流量"),
    ("Wall heat", "壁面热流"),
    ("Warning", "警告"),
    ("What happened", "发生了什么"),
    ("What is sent", "发送的内容"),
    ("Where the slice crosses its axis", "切片与其轴的交点位置"),
    ("Without saving, they are lost.", "不保存将丢失。"),
    ("Worst: element {worst}{}", "最差：单元 {worst}{}"),
    ("Write Deck", "写入输入文件"),
    ("Write the node's results to a CSV file", "将节点结果写入 CSV 文件"),
    ("Write the reaction sums to a CSV file", "将反力合计写入 CSV 文件"),
    ("Write the series to a CSV file", "将序列写入 CSV 文件"),
    ("Wrote {}", "已写入 {}"),
    ("Wrote {} ({name}{units})", "已写入 {}（{name}{units}）"),
    ("XY Plot", "曲线"),
    ("Zoom", "缩放"),
    ("Zoom: left drag up and down (or the wheel)", "缩放：左键上下拖动（或滚轮）"),
    ("\\nClick to roll the model to here", "\\n单击可将模型回滚到此处"),
    (
        "\\nOpened from a format {n} file: the first save keeps that file as a copy beside it",
        "\\n从格式 {n} 文件打开：首次保存会在其旁边保留该文件的副本",
    ),
    ("\\nThese commands are {BEYOND_HISTORY}", "\\n这些命令 {BEYOND_HISTORY}"),
    ("\\nThis command is {BEYOND_HISTORY}", "\\n此命令 {BEYOND_HISTORY}"),
    ("a new model", "新模型"),
    ("an Abaqus deck", "Abaqus 输入文件"),
    ("at node {node}", "节点 {node} 处"),
    ("at {}", "在 {}"),
    ("boundary condition", "边界条件"),
    ("by {}", "由 {}"),
    ("by {} · suppressed", "由 {} · 已抑制"),
    ("cancelled: {}", "已取消：{}"),
    ("click to fold them", "单击以折叠"),
    ("click to show each one", "单击以逐个显示"),
    ("face, ", "面，"),
    ("fieldcraft {command}", "fieldcraft {command}"),
    ("final state", "最终状态"),
    ("first load factor {}", "首个载荷因子 {}"),
    ("for {}", "用于 {}"),
    ("frame {k}", "帧 {k}"),
    ("initial {}", "初始 {}"),
    ("just now", "刚刚"),
    ("load factor {}", "载荷因子 {}"),
    ("max Mises", "最大 Mises"),
    ("max T", "最高 T"),
    ("max |U|", "最大 |U|"),
    ("mesh of", "的网格"),
    ("min {} · mean {} · max {}", "最小 {} · 平均 {} · 最大 {}"),
    ("mode 1 at {} Hz", "振型 1，{} Hz"),
    ("mode {mode}", "模式 {mode}"),
    ("needs {}", "需要 {}"),
    ("no bodies (open a design first)", "无实体（请先打开设计）"),
    ("no command matches `{line}`", "没有匹配 `{line}` 的命令"),
    ("no {format} deck writer yet", "尚无 {format} 输入文件写入器"),
    ("of {}", "基于 {}"),
    (
        "older than the undo history (it came with the opened model, or is past the undo memory), so it cannot be undone",
        "早于撤销历史（随打开的模型而来，或已超出撤销记忆），因此无法撤销",
    ),
    ("on {}", "在 {} 上"),
    ("ready to solve, no issues", "可求解，无问题"),
    ("render `{}`", "渲染 `{}`"),
    ("rigid wall", "刚性墙"),
    ("seeds on", "种子位于"),
    ("shown one by one while the model stands among them", "模型处于其中时逐个显示"),
    ("size {h}", "大小 {h}"),
    ("soon · {m}", "即将 · {m}"),
    ("the parameters must be JSON: {e}", "参数必须是 JSON：{e}"),
    ("the series", "序列"),
    ("the step", "分析步"),
    ("the update", "更新"),
    ("the {} sample", "{} 示例"),
    ("total reaction force ({})", "总反力（{}）"),
    ("touching pair", "接触对"),
    ("{bodies} bodies", "{bodies} 个几何体"),
    ("{busy}: wait for it or cancel it", "{busy}：请等待或取消"),
    ("{done} in {took}", "{done}，用时 {took}"),
    ("{done}: {n} {kind}{q} in {took} — Mesh › Quality checks them", "{done}：{n} 个 {kind}{q}，用时 {took} — 网格 › 质量会检查"),
    ("{field} on a slice at {axis} = {}", "{axis} = {} 切片上的 {field}"),
    ("{job} is running", "{job} 正在运行"),
    ("{job} is running: wait for it or cancel it", "{job} 正在运行：请等待或取消"),
    ("{kind} · suppressed", "{kind} · 已抑制"),
    ("{label} (sample)", "{label}（示例）"),
    ("{label} done", "{label} 完成"),
    ("{missing}\\nA field holds something that is not a value", "{missing}\\n某个字段包含的不是有效值"),
    ("{name} (stale)", "{name}（已过期）"),
    ("{n} feature{}", "{n} 个特征{}"),
    ("{n} {}{} picked in the view (Select: {}s)", "视图中已拾取 {n} 个{}{}（选择：{}s）"),
    ("{over} of {limited} components over their limit", "{limited} 个分量中有 {over} 个超出限值"),
    (
        "{path}: FieldZ opens .fieldcraft models, .solidcraft designs, .3mf tagged meshes and .msh orphan meshes",
        "{path}：FieldZ 可打开 .fieldcraft 模型、.solidcraft 设计、.3mf 标记网格和 .msh 孤立网格",
    ),
    ("{step} (stale)", "{step}（已过期）"),
    ("{v:.0} pt", "{v:.0} pt"),
    ("{valid}/{} values · {} gaps", "{valid}/{} 个值 · {} 处间断"),
    ("{what} is used by {}: change or delete those first.", "{what} 被 {} 使用：请先修改或删除它们。"),
    ("{what} × {} (display interpolation)", "{what} × {}（显示插值）"),
    ("{} ({} nodes)", "{}（{} 个节点）"),
    ("{} DOFs", "{} 自由度"),
    ("{} KB", "{} KB"),
    ("{} again, with the same parameters", "再次 {}，使用相同参数"),
    ("{} among {}", "{}（共 {}）"),
    ("{} at a node", "节点处的 {}"),
    ("{} bodies", "{} 个实体"),
    ("{} building {}", "{} 正在构建 {}"),
    ("{} cancelled", "{} 已取消"),
    ("{} crash report(s) since the last start: {}", "自上次启动以来 {} 份崩溃报告：{}"),
    ("{} days ago", "{} 天前"),
    ("{} h ago", "{} 小时前"),
    ("{} has none of the bodies the model is on", "{} 没有模型所在的任何实体"),
    ("{} has none of the bodies {}'s mesh and sets are on.", "{} 没有 {} 的网格和集合所在的任何实体。"),
    ("{} items: {}, …", "{} 项：{}，…"),
    ("{} min ago", "{} 分钟前"),
    ("{} min {:02} s", "{} 分 {:02} 秒"),
    ("{} nodes, {} elements", "{} 个节点，{} 个单元"),
    ("{} seeds", "{} 个种子"),
    ("{} still to do — {}", "{} 仍待完成 — {}"),
    ("{} traced", "已追踪 {}"),
    ("{} · absorbed into {into}", "{} · 已并入 {into}"),
    ("{} · by {}", "{} · 由 {}"),
    ("{} · soon ({m})", "{} · 即将推出（{m}）"),
    ("{} · {} faces", "{} · {} 个面"),
    ("{}, filled in", "{}，已填写"),
    ("{}, {}, units {}", "{}，{}，单位 {}"),
    ("{}: its form with these parameters", "{}：使用这些参数的表单"),
    ("{}: not yet", "{}：尚未"),
    ("{}: stopped unexpectedly", "{}：意外停止"),
    ("{}: {} commands, {what}", "{}：{} 条命令，{what}"),
    ("… {} more", "… 另有 {} 项"),
];

/// The Chinese text for `text` when the thread's language is `zh`, else `text` itself.
pub fn ts(text: &'static str) -> &'static str {
    if solidcraft_ui_text::language() != "zh" {
        return text;
    }
    lookup(text).unwrap_or(text)
}

/// [`ts`] as a `Cow`, for call sites that build owned text from it.
pub fn tr(text: &'static str) -> Cow<'static, str> {
    Cow::Borrowed(ts(text))
}

/// [`ts`] as an owned `String`, for a field or parameter that takes one.
pub fn owned(text: &'static str) -> String {
    ts(text).to_owned()
}

/// The Chinese text for an engine catalogue label: `id` is the entry's id under `cap.` in
/// `locales/en.json` and `locales/zh.json` (`mesh.tet.generate`), `english` its label in the
/// current catalogue. A label with no entry (or an English thread) stays `english`.
pub fn cap<'a>(id: &str, english: &'a str) -> &'a str {
    let key = format!("cap.{id}");
    if let Some(text) = messages(solidcraft_ui_text::locale().catalog()).get(&key) {
        return text;
    }
    if let Some(text) = messages(solidcraft_ui_text::Locale::for_language("en").catalog()).get(&key) {
        return text;
    }
    english
}

/// `messages` of a catalogue that lives for the process (the catalogues are installed once).
fn messages(catalog: &'static solidcraft_ui_text::Catalog) -> &'static solidcraft_ui_text::Messages {
    catalog.messages()
}

/// A panel's label (`cap.panel.<slug>`).
pub fn panel(label: &'static str) -> &'static str {
    cap(&format!("panel.{}", slug(label)), label)
}

/// A tab's label (`cap.tab.<id>`).
pub fn tab(id: &str, label: &'static str) -> &'static str {
    cap(&format!("tab.{id}"), label)
}

/// A workspace's label (`cap.workspace.label.<slug>`).
pub fn workspace(label: &'static str) -> &'static str {
    cap(&format!("workspace.label.{}", slug(label)), label)
}

/// `text` with every character outside ASCII letters and digits turned into `_` and letters
/// lower-cased, the id fragment a panel or workspace name gets.
pub fn slug(text: &str) -> String {
    text.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect()
}

/// The Chinese entry for `text`, if there is one.
fn lookup(text: &str) -> Option<&'static str> {
    CHROME.binary_search_by(|(english, _)| (*english).cmp(text)).ok().map(|i| CHROME[i].1)
}

/// [`fill`] with the English template `$t` and the same arguments `format!` takes.
macro_rules! wfmt {
    ($t:literal $($rest:tt)*) => {
        $crate::words::fill($t, format!($t $($rest)*))
    };
}
pub(crate) use wfmt;

/// The English `template` formatted as `formatted` (by `format!`), with the same values in the
/// Chinese template when the thread's language is `zh`. Falls back to `formatted` for a template
/// with no Chinese entry, a different placeholder count, or a value that does not line up.
pub fn fill(template: &'static str, formatted: String) -> String {
    if solidcraft_ui_text::language() != "zh" {
        return formatted;
    }
    let Some(chinese) = lookup(template) else { return formatted };
    splice(template, chinese, &formatted).unwrap_or(formatted)
}

/// One piece of a format template: literal text, or a placeholder's contents (`name` or
/// `name:spec`).
enum Piece<'a> {
    Lit(&'a str),
    Hole(&'a str),
}

/// Split a format template into literals and placeholders, keeping `{{`/`}}` as literal braces.
fn pieces(template: &str) -> Vec<Piece<'_>> {
    let bytes = template.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    let mut start = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'{' if bytes.get(i + 1) == Some(&b'{') => {
                if start < i {
                    out.push(Piece::Lit(&template[start..i]));
                }
                out.push(Piece::Lit(&template[i..i + 1]));
                i += 2;
                start = i;
            }
            b'{' => {
                if start < i {
                    out.push(Piece::Lit(&template[start..i]));
                }
                let Some(close) = template[i + 1..].find('}') else {
                    i += 1;
                    continue;
                };
                let end = i + 1 + close;
                out.push(Piece::Hole(&template[i + 1..end]));
                i = end + 1;
                start = i;
            }
            b'}' if bytes.get(i + 1) == Some(&b'}') => {
                if start < i {
                    out.push(Piece::Lit(&template[start..i]));
                }
                out.push(Piece::Lit(&template[i..i + 1]));
                i += 2;
                start = i;
            }
            _ => i += 1,
        }
    }
    if start < bytes.len() {
        out.push(Piece::Lit(&template[start..]));
    }
    out
}

/// The values `format!` put in `english`'s placeholders, in order. `None` when a literal
/// segment is not found (the value contained it) or two placeholders touch.
fn values<'a>(english: &str, formatted: &'a str) -> Option<Vec<&'a str>> {
    let parts = pieces(english);
    let mut out = Vec::new();
    let mut pos = 0;
    for (index, part) in parts.iter().enumerate() {
        match part {
            Piece::Lit(literal) => {
                let at = formatted.get(pos..)?.find(literal)?;
                pos += at + literal.len();
            }
            Piece::Hole(_) => {
                let next = match parts.get(index + 1) {
                    Some(Piece::Lit(literal)) => Some(*literal),
                    Some(Piece::Hole(_)) => return None,
                    None => None,
                };
                match next {
                    Some(literal) => {
                        let at = formatted.get(pos..)?.find(literal)?;
                        out.push(&formatted[pos..pos + at]);
                        pos += at;
                    }
                    None => {
                        out.push(formatted.get(pos..)?);
                        pos = formatted.len();
                    }
                }
            }
        }
    }
    Some(out)
}

/// `english`'s values in `chinese`'s placeholders: a named placeholder takes the value with the
/// same name, a positional one the next positional value (a translation may move a named
/// placeholder but keeps positional ones in order).
fn splice(english: &str, chinese: &str, formatted: &str) -> Option<String> {
    // Two placeholders with nothing between them (`{}{frame}`) cannot be told apart by value;
    // take the text the whole run formatted to and put it back at the same run.
    if hole_runs(english).iter().any(|run| run.len() > 1) || hole_runs(chinese).iter().any(|run| run.len() > 1) {
        return splice_grouped(english, chinese, formatted);
    }
    let english_parts = pieces(english);
    let chinese_parts = pieces(chinese);
    let english_holes: Vec<&str> = english_parts
        .iter()
        .filter_map(|p| match p {
            Piece::Hole(name) => Some(*name),
            Piece::Lit(_) => None,
        })
        .collect();
    let chinese_holes = chinese_parts.iter().filter(|p| matches!(p, Piece::Hole(_))).count();
    if english_holes.len() != chinese_holes {
        return None;
    }
    let values = values(english, formatted)?;
    // The values of the positional holes, in their order in the English template.
    let positional: Vec<&str> = english_holes.iter().enumerate().filter(|(_, name)| base_name(name).is_empty()).map(|(at, _)| values[at]).collect();
    let mut out = String::with_capacity(chinese.len() + formatted.len() / 4);
    let mut next = 0;
    for part in &chinese_parts {
        match part {
            Piece::Lit(literal) => out.push_str(literal),
            Piece::Hole(name) => {
                let base = base_name(name);
                let value = if base.is_empty() {
                    let value = positional.get(next)?;
                    next += 1;
                    value
                } else {
                    let at = english_holes.iter().position(|english| base_name(english) == base)?;
                    values.get(at)?
                };
                out.push_str(value);
            }
        }
    }
    Some(out)
}

/// A placeholder's name without its format spec (`d` from `d:.1`).
fn base_name(hole: &str) -> &str {
    hole.split(':').next().unwrap_or("")
}

/// The placeholder names of each run of adjacent placeholders in `template`.
fn hole_runs(template: &str) -> Vec<Vec<&str>> {
    let parts = pieces(template);
    let mut runs = Vec::new();
    let mut i = 0;
    while i < parts.len() {
        match &parts[i] {
            Piece::Hole(name) => {
                let mut run = vec![base_name(name)];
                i += 1;
                while let Some(Piece::Hole(name)) = parts.get(i) {
                    run.push(base_name(name));
                    i += 1;
                }
                runs.push(run);
            }
            Piece::Lit(_) => i += 1,
        }
    }
    runs
}

/// What each run of adjacent placeholders in `english` formatted to, in order.
fn run_values<'a>(english: &str, formatted: &'a str) -> Option<Vec<&'a str>> {
    let parts = pieces(english);
    let mut out = Vec::new();
    let mut pos = 0;
    let mut i = 0;
    while i < parts.len() {
        match &parts[i] {
            Piece::Lit(literal) => {
                let at = formatted.get(pos..)?.find(literal)?;
                pos += at + literal.len();
                i += 1;
            }
            Piece::Hole(_) => {
                let mut j = i;
                while matches!(parts.get(j), Some(Piece::Hole(_))) {
                    j += 1;
                }
                match parts.get(j) {
                    Some(Piece::Lit(literal)) => {
                        let at = formatted.get(pos..)?.find(literal)?;
                        out.push(formatted.get(pos..pos + at)?);
                        pos += at;
                    }
                    Some(Piece::Hole(_)) => return None,
                    None => {
                        out.push(formatted.get(pos..)?);
                        pos = formatted.len();
                    }
                }
                i = j;
            }
        }
    }
    Some(out)
}

/// `english`'s run values in `chinese`'s runs: a run with the same placeholder names takes that
/// run's value, else its own position's.
fn splice_grouped(english: &str, chinese: &str, formatted: &str) -> Option<String> {
    let english_runs = hole_runs(english);
    let chinese_runs = hole_runs(chinese);
    if english_runs.len() != chinese_runs.len() {
        return None;
    }
    let values = run_values(english, formatted)?;
    if values.len() != english_runs.len() {
        return None;
    }
    let mut taken = vec![false; english_runs.len()];
    let mut chosen = Vec::with_capacity(chinese_runs.len());
    for (at, run) in chinese_runs.iter().enumerate() {
        let pick = english_runs.iter().position(|english| english == run).unwrap_or(at);
        if pick >= values.len() || taken[pick] {
            return None;
        }
        taken[pick] = true;
        chosen.push(pick);
    }
    let parts = pieces(chinese);
    let mut out = String::with_capacity(chinese.len() + formatted.len() / 4);
    let mut run = 0;
    let mut i = 0;
    while i < parts.len() {
        match &parts[i] {
            Piece::Lit(literal) => {
                out.push_str(literal);
                i += 1;
            }
            Piece::Hole(_) => {
                out.push_str(values[chosen[run]]);
                run += 1;
                i += 1;
                while matches!(parts.get(i), Some(Piece::Hole(_))) {
                    i += 1;
                }
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every chrome literal has a Chinese entry, and the table is sorted for the binary search.
    #[test]
    fn the_chrome_table_is_sorted_and_every_literal_is_translated() {
        assert!(!CHROME.is_empty());
        assert!(CHROME.windows(2).all(|w| w[0].0 < w[1].0), "CHROME must be sorted by its English text");
        for (english, chinese) in CHROME {
            assert!(!english.is_empty(), "an empty English literal");
            assert!(!chinese.trim().is_empty(), "{english} has no Chinese text");
        }
        // A few read the same in both languages: key names, units and other products' names.
        // Every other literal has text of its own.
        let as_is = ["Alias", "Cmd", "Ctrl", "Enter", "FieldZ {}", "Fusion", "Inventor", "Tinkercad", "fieldcraft {command}", "{v:.0} pt", "{} KB"];
        let untranslated: Vec<&str> = CHROME.iter().filter(|(english, chinese)| english == chinese).map(|(english, _)| *english).collect();
        assert_eq!(untranslated, as_is, "every other chrome literal has Chinese text of its own");
    }

    /// English outside a scope, Chinese inside one; a template keeps its values.
    #[test]
    fn the_language_switches_and_templates_keep_their_values() {
        crate::theme::install_look();
        assert_eq!(ts("Solve"), "Solve");
        {
            let _zh = solidcraft_ui_text::scope("zh");
            assert_eq!(ts("Solve"), "求解");
            assert_eq!(tr("File").as_ref(), "文件");
            assert_eq!(owned("Nothing to redo"), "没有可重做的操作");
            assert_eq!(fill("Save ({})", "Save (Ctrl+S)".into()), "保存（Ctrl+S）");
            assert_eq!(fill("Update to {version}", "Update to 1.2".into()), "更新到 1.2");
            assert_eq!(fill("{valid}/{} values · {} gaps", "3/4 values · 1 gaps".into()), "3/4 个值 · 1 处间断");
            // A translation may put a named placeholder before another; a positional one keeps its value.
            assert_eq!(fill("Reaction {upper} on {set}", "Reaction 12 N on Base".into()), "Base 上的反力 12 N");
            assert_eq!(fill("{field} on a slice at {axis} = {}", "Stress on a slice at Y = 3".into()), "Y = 3 切片上的 Stress");
            // Two placeholders with nothing between them keep the text their run made.
            assert_eq!(fill("Deformation × {}{how}", "Deformation × 2.0（自动）".into()), "变形 × 2.0（自动）");
            assert_eq!(fill("Saved {}{}", "Saved 3 files".into()), "已保存 3 files");
        }
        assert_eq!(ts("Solve"), "Solve");
    }

    /// A catalogue label reads its `cap.<id>` entry, in either language, and falls back.
    #[test]
    fn catalogue_labels_read_their_cap_entry() {
        crate::theme::install_look();
        assert_eq!(cap("mesh.tet.generate", "Tet Mesh"), "Tet Mesh");
        {
            let _zh = solidcraft_ui_text::scope("zh");
            assert_eq!(cap("mesh.tet.generate", "Tet Mesh"), "四面体网格");
            assert_eq!(cap("no.such.entry", "Fallback"), "Fallback");
        }
    }
}
