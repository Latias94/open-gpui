# 原生 UI 通用框架应该如何设计 - 调研汇总

- 生成时间：2026-07-02 15:42:47
- 结果目录：`F:\SourceCodes\Rust\open-gpui\native-ui-framework-design-research\results`
- 样本数量：28

## 目录

1. [shadcn/ui](#shadcn-ui) - open_gpui_relevance: 建议为 trial：采用 shadcn/ui 的复制后自有、registry、CLI、docs 和 AI-friendly 思路做 open-gpui 试点，但不要采用其 Web 运行时和... | must_have_for_open_gpui: open-gpui 必须优先补齐的是源码可拥有的组件 scaffold、机器可读 registry、基础 anatomy、主题 token、overlay primitive、表单基础控件、...
2. [Radix UI Primitives](#radix-ui-primitives) - open_gpui_relevance: 建议定位为 reference-only，并对 overlay、focus、keyboard、component anatomy 做 trial。不要采用 React API 或 DOM 绑... | must_have_for_open_gpui: 必须借鉴的是组件 anatomy、受控/非受控模型、键盘表、焦点管理、dismiss/layering、overlay collision、可访问性 contract、state-to-st...
3. [Floating UI](#floating-ui) - open_gpui_relevance: adopt：采纳 Floating UI 的设计内核和术语，但不要直接移植实现。open-gpui 应设计一个原生 OverlayBehaviorKernel：GeometryPlatfor... | must_have_for_open_gpui: 必须补齐。一个通用原生 UI 框架如果没有统一 overlay positioning/collision/interaction kernel，后续 tooltip、dropdown、co...
4. [Floating UI Rust crates](#floating-ui-rust-crates) - open_gpui_relevance: 最终建议：reference-only，辅以小范围 trial。不要直接 adopt `floating-ui-dom`，也不建议 fork 整个 RustForWeb 项目；可以短期用 `... | must_have_for_open_gpui: 必须补齐的不是对 RustForWeb crate 的依赖，而是同等级的 overlay positioning 能力：flip、shift、collision padding、size c...
5. [React Aria / React Aria Components](#react-aria-react-aria-components) - open_gpui_relevance: 建议 reference-only + targeted trial：不要采用 React/DOM API，也不要复制完整组件面；应把 React Aria 作为 a11y、interact... | must_have_for_open_gpui: 必须借鉴的是 a11y 行为系统化、状态层分离、collection/selection 模型、slot/anatomy、受控/非受控状态、press/focus/hover 统一事件、ov...
6. [Zag.js](#zag-js) - open_gpui_relevance: 建议 adopt 核心思想、trial 少量机器、reject Web 运行时形态。Zag.js 是 open-gpui 设计 renderer-neutral UI primitive c... | must_have_for_open_gpui: 必须借鉴的是状态机优先的 primitive 设计、`machine`/`connect`/adapter 分层、parts anatomy、controlled/uncontrolled...
7. [Ark UI](#ark-ui) - open_gpui_relevance: 建议为 trial 偏 adopt：不要采用 Ark 的 Web 运行时，但应采用它的架构思想。直接设计含义是 open-gpui 应优先设计 renderer-neutral 状态机/行为... | must_have_for_open_gpui: 必须重点借鉴的是 Zag 式有限状态机、跨 adapter 分层、Root/Trigger/Content/Item/Positioner anatomy、受控/非受控/RootProvid...
8. [Base UI](#base-ui) - open_gpui_relevance: 建议为 trial：把 Base UI 作为 Radix 后继/竞争参考，试点其 anatomy、event details、Positioner、state-to-style、Markdo... | must_have_for_open_gpui: 必须借鉴的是 anatomy/parts、无样式边界、受控/非受控状态、event reason/cancel、render/slot 替换、Positioner、focus managem...
9. [fret](#fret) - open_gpui_relevance: 建议：reference-only + trial。Fret 不应被 open-gpui 直接 adopt，但应作为本地高价值设计参考：采纳机制/策略分层、headless primitiv... | must_have_for_open_gpui: 必须借鉴三类能力：1. 机制/策略边界和 headless 状态机层；2. shadcn/Radix-like component anatomy 与 typed facade/raw es...
10. [gpui-component](#gpui-component) - open_gpui_relevance: 最终建议是仅参考，局部试点采用。open-gpui 应参考 gpui-component 的组件覆盖、Root layer、主题 registry、VirtualList/DataTable... | must_have_for_open_gpui: open-gpui 必须优先补齐的是底层通用能力：Root/layer 管理、Dialog/Sheet/Popover/Tooltip/Menu 的 overlay/focus/dismis...
11. [Zed UI / GPUI](#zed-ui-gpui) - open_gpui_relevance: 建议为 reference-only 加 targeted adopt：不要直接采用 Zed UI 作为 open-gpui 组件库，但应定向吸收其生产经验。直接设计含义是：建立 GPUI-... | must_have_for_open_gpui: 必须吸收的是：`prelude` 和 traits 统一 API；强类型 builder 组件；ButtonLike/ListItem 这类可组合底座；Component trait、met...
12. [SwiftUI](#swiftui) - open_gpui_relevance: 建议为 reference-only，并对 state/binding/environment、modifier、preview、semantic controls 和 accessibil... | must_have_for_open_gpui: 对 open-gpui 必须借鉴的是声明式组合、轻量 View/Element 描述、modifier ergonomics、state/binding/environment、single...
13. [Jetpack Compose](#jetpack-compose) - open_gpui_relevance: 最终建议为 reference-only 偏 trial：不要采用 Compose 的 Android 平台实现，但应强参考其声明式 API、state hoisting、modifier、... | must_have_for_open_gpui: 对 open-gpui 必须吸收的能力是声明式 composable/builder 体验、modifier-like 可组合修饰器、state hoisting 与单一事实源、slot-b...
14. [Flutter](#flutter) - open_gpui_relevance: 建议 reference-only + targeted trial。不要采用 Flutter 的 Dart SDK、移动优先平台层和全量组件路线；应重点试验 widget/element/... | must_have_for_open_gpui: 必须借鉴的是 widget/element/render 分层、不可变 UI 描述 + 持久运行时对象、constraints layout、sliver/virtual scrolling...
15. [Slint](#slint) - open_gpui_relevance: 建议为 trial + reference-only：Slint 的工具链、静态 UI 合约、属性绑定、model、状态动画、预览、测试和多后端抽象值得试点参考；DSL-first、多语言绑... | must_have_for_open_gpui: 必须借鉴的是静态可分析组件 contract、强错误诊断、实时预览/示例驱动、属性绑定语义、model-driven list、状态/动画表达、可访问性属性、跨后端抽象纪律和构建期验证。op...
16. [Iced](#iced) - open_gpui_relevance: 建议为 reference-only 加 targeted adopt。不要采用 Iced 作为 open-gpui 的架构母体，也不要复制其完整应用框架；应定向吸收 Rust/Cargo... | must_have_for_open_gpui: 必须借鉴的是 Elm-style 清晰状态模型、Task/Subscription 副作用边界、renderer/runtime/widget/window crate 分层、feature...
17. [egui](#egui) - open_gpui_relevance: 最终建议：reference-only + targeted adopt。不要采用 egui 作为 open-gpui 通用组件架构母体，也不要把主组件框架改成 immediate mode... | must_have_for_open_gpui: 对 open-gpui 必须借鉴的是低摩擦 Rust API、可嵌入 renderer 边界、custom widget/painter 体验、web demo/gallery 的可试用性、...
18. [Xilem / Linebender UI](#xilem-linebender-ui) - open_gpui_relevance: 建议为 reference-only + targeted trial。不要 adopt Xilem/Masonry 作为 open-gpui 架构母体，也不要复制其完整 API；应定向试验... | must_have_for_open_gpui: 必须吸收的是架构思想而非完整 API：轻量 view tree 与 retained element tree 分离、`build/rebuild/message` 式 reconcilia...
19. [Makepad](#makepad) - open_gpui_relevance: 建议为 reference-only + targeted trial。不要采用 Makepad 作为 open-gpui 架构母体，也不要直接引入 DSL-first 平台路线；应定向试验... | must_have_for_open_gpui: 必须借鉴的是 Rust-native GPU 自绘信心、Live/Splash 式可读 UI 描述、热更新反馈循环、Studio/inspector 对设计开发协作的价值、shader 与...
20. [Tauri](#tauri) - open_gpui_relevance: 建议 reference-only + interoperability trial。不要采用 Tauri 的 WebView 渲染路线作为 open-gpui 主路径，也不要追 Tauri... | must_have_for_open_gpui: open-gpui 必须补齐的不是 Tauri 的 WebView UI，而是可互操作的应用壳能力和工程工具：窗口/多窗口基础、菜单、托盘、快捷键、文件对话框、通知、deep link、更新...
21. [Electron](#electron) - open_gpui_relevance: 建议 reference-only + targeted interoperability trial。不要采用 Electron 的 Chromium/WebView 渲染路线作为 ope... | must_have_for_open_gpui: open-gpui 必须补齐的不是 Electron 的 WebView 渲染路线，而是它证明用户会期待的桌面工程闭环：窗口/多窗口、菜单、上下文菜单、托盘、快捷键、文件对话框、通知、剪贴板...
22. [TanStack Table / TanStack Virtual](#tanstack-table-tanstack-virtual) - open_gpui_relevance: 建议为 trial：架构原则应 adopt，具体 TypeScript API 只做 reference-only。优先做一个 `table_core + virtualizer_core... | registry_viability: TanStack 证明复杂行为库不一定需要源码 registry；对 native Rust UI，更可行的是 crates.io 上的 headless core + adapter cr... | must_have_for_open_gpui: 必须补齐的是 headless table/tree/list core、统一 row/column/cell/header anatomy、可控状态片段、服务端/manual 模式、扩展...
23. [Storybook / Chromatic](#storybook-chromatic) - open_gpui_relevance: 建议 adopt 工具链思想、trial native gallery runner、defer 云端协作平台。直接设计含义是：open-gpui 的通用 UI 框架不应让 native g... | must_have_for_open_gpui: 必须补齐，但应作为工具链能力而不是核心渲染 API。open-gpui 通用 UI 框架至少需要本地 gallery、story manifest、可交互 examples、文档派生、截图基...
24. [Design Tokens Community Group / Style Dictionary](#design-tokens-community-group-style-dictionary) - open_gpui_relevance: 建议采纳（adopt）其核心思想，并试点（trial）Rust 原生实现。直接设计含义是：open-gpui 应定义一个 DTCG-like theme source schema，保留 $... | must_have_for_open_gpui: 必须补齐。一个通用 open-gpui UI 框架如果没有正式 theme token schema 和生成/校验管线，组件库会很快出现硬编码颜色、重复尺寸、暗色模式漂移、第三方主题不可验证...
25. [AccessKit](#accesskit) - open_gpui_relevance: 最终建议：adopt。AccessKit 应成为 open-gpui native accessibility contract 的主要目标后端和测试基准，但不要让组件直接裸写 Access... | must_have_for_open_gpui: 对 open-gpui 来说必须补齐，并且应该尽早进入架构核心。最低要求是：定义 renderer-neutral semantics tree；所有基础组件必须声明 AccessKit m...
26. [Cargo / crates.io / cargo-generate / xtask scaffold](#cargo-crates-io-cargo-generate-xtask-scaffold) - open_gpui_relevance: 最终建议：adopt。open-gpui 应把 Cargo/crates.io/workspace/features/SemVer 作为主分发底座，把 cargo-generate 作为新项... | registry_viability: 对 native Rust UI 来说，不需要复刻 shadcn 的完整源码 registry 作为主分发渠道。Cargo/crates.io 已经是 Rust 的 package regi... | must_have_for_open_gpui: 对 open-gpui 来说是必须补齐的基础能力：稳定 crate 分层和发布顺序、Cargo features 策略、cargo add 友好的 README/docs、cargo-gen...
27. [AI-era component distribution](#ai-era-component-distribution) - open_gpui_relevance: 建议 adopt 核心思想、trial 最小闭环。直接设计含义是：open-gpui 应把 AI-era component distribution 作为生态基础设施，而不是后期文档补丁。... | must_have_for_open_gpui: 必须补齐。open-gpui 通用 UI 框架至少需要：registry schema、components manifest、gpui add/diff/verify、源码 recipe、...
28. [Hybrid registry model](#hybrid-registry-model) - open_gpui_relevance: 建议 trial：不要做 shadcn 式源码 registry；先做 hybrid registry MVP。范围为导出当前 component_contract/theme/a11y/g... | registry_viability: registry 有必要，但不应是 shadcn 式源码 registry 的简单移植。更合适的是 metadata registry：记录组件名称、owner、family、gallery... | must_have_for_open_gpui: 必须补的是 machine-readable public manifest、recipe/scaffold schema、registry-to-docs/gallery 派生、third...

## 详细内容

## <a id="shadcn-ui"></a>1. shadcn/ui

- 结果文件：`shadcn_ui.json`
- 调研类别：`frontend_component_distribution`
- 纳入原因：
  copy-to-own、registry、CLI、docs 与 AI-friendly 组件分发的核心参考；需要判断这种模式迁移到 Rust/native UI 是否仍成立。
- 参考来源：
  - https://ui.shadcn.com/docs
  - https://ui.shadcn.com/docs/registry

### 定位

#### `positioning`

> shadcn/ui 的定位是前端源码型组件分发平台：提供 React、Tailwind CSS、Radix UI 或 Base UI 组件、CLI、components.json、registry schema、社区
> directory、MCP 与面向 AI 的资料。它不是传统黑盒组件包，而是把组件源码复制到应用项目中，由应用团队拥有并继续演进。

#### `target_users`

主要服务 Web 应用开发者、设计系统作者、产品工程师、组件生态维护者和 AI agent；对 open-gpui 的参考对象是希望快速搭建原生桌面 UI 的 Rust 应用团队和框架维护者。

#### `primary_value_proposition`

> 核心价值是用可读源码、好默认值、可组合结构和机器可读 registry 降低组件采用门槛。与 open-gpui 匹配的是分发、可定制和 AI 友好理念；不直接匹配的是 Web、Tailwind、DOM 和 React 运行时依赖。

### 分发与生态

#### `distribution_model`

> 采用复制后自有的源码分发模型。官方 CLI 负责 init、add、diff、build、search、view、mcp 等工作流；components.json 记录样式、Tailwind、别名、图标库、RTL 和 registry
> 配置；registry.json 描述组件集合，registry-item.json 描述单个条目的类型、依赖、文件、目标路径、CSS、CSS
> 变量、文档、分类和元数据。分发单位可以是组件、hook、页面、block、主题、字体、基础样式或任意文件，并支持命名空间 registry、远程 registry、私有 registry 和社区 directory。

#### `source_ownership`

> 用户在项目内拥有生成后的组件源码，可以直接改结构、样式、依赖和行为，避免被上游组件库 API 锁死。成本是升级不再是普通包升级，而是类似补丁合并：需要用 CLI diff、重新 add、人工 review 或 AI
> 辅助合并处理本地改动与上游更新的漂移。

### AI 时代设计

#### `ai_friendliness`

> shadcn/ui 对 AI 很友好：源码可读、组件粒度清晰、文档公开、llms.txt 提供入口索引、MCP 允许 AI 浏览和安装组件，skills 提供面向 AI 助手的领域知识，registry schema
> 让组件、依赖和文件目标路径可机器读取。对 open-gpui 的启发是：不要只写人看的 docs，还要提供可检索、可组合、可验证的组件元数据和示例。

#### `machine_readable_contracts`

> 它已经具备较强的机器可读基础：components.json 约束项目配置，registry.json 描述 registry 根索引，registry-item.json
> 描述条目类型、dependencies、devDependencies、registryDependencies、files、target、tailwind、cssVars、css、envVars、docs、categories、meta
> 等。open-gpui 应在此基础上增加 Rust 类型信息、feature graph、可访问性语义、焦点行为、键盘交互、布局约束、截图基线和性能预算。

### API 与组合

#### `api_ergonomics`

> API 体验偏声明式组合：React 组件导出后以 Root、Trigger、Content、Item 等部件组合，简单组件则通过 props、variant 和 className 定制。因为源码在本地，escape hatch
> 很直接。迁移到 GPUI 时，等价形态应是 typed builder、Element 组合、Entity 状态托管、事件回调和 recipe-driven 默认样式，而不是把 React hook 模型逐字搬过来。

#### `customization_model`

> 定制模型分多层：components.json 决定全局样式和路径别名；CSS 变量和 Tailwind 控制主题；组件源码、className、variant、子组件和依赖可直接修改；registry item 可携带
> css、cssVars、依赖和文件。open-gpui 可映射为 theme token、recipe、组件 prop、局部源码 override 和 app adapter 五层，并明确覆盖优先级。

#### `component_anatomy_model`

> 复杂组件普遍采用 anatomy 拆分，例如 dialog、dropdown、select、tabs、accordion、tooltip、popover 等由
> root、trigger、content、item、separator、indicator、portal 等部件组合；简单组件如 button、badge、card 则是可复制的源码封装。这个模型适合 open-gpui，因为 GPUI
> 可以把行为 primitive、渲染 Element、状态 Entity 和样式 recipe 分开暴露。

### Headless 与行为

#### `headless_boundary`

> shadcn/ui 不是纯 headless 系统，而是把 headless primitive、可访问性行为、React adapter、Tailwind 样式和本地源码组合在一起。边界优点是上手快，缺点是行为和样式容易耦合。open-
> gpui 应吸收其 anatomy 和源码所有权，但把行为逻辑、状态机、AccessKit metadata、定位服务、渲染 adapter、theme recipe 分层得更硬。

### 渲染与性能

#### `rendering_model`

> 渲染模型是 React 组件加 DOM，加 Tailwind CSS 和 CSS 变量，由浏览器布局、绘制、滚动和可访问性树承担运行时能力。shadcn/ui 自身不是渲染框架，也不提供 retained native、immediate
> mode 或 GPU scene 模型。

#### `native_advantage`

> native GPUI 应在大文本编辑、低延迟输入、长列表和树、复杂 docking、原生窗口集成、GPU 绘制、低内存常驻、跨窗口状态和桌面快捷键体验上胜过 WebView 或 DOM。shadcn/ui
> 的模式可以帮助这些能力被组件化分发，但不能替代底层性能设计。

#### `web_ecosystem_advantage`

> Web/Tauri/Electron 生态在表单库、图表、营销页面、仪表盘 blocks、Radix 级可访问性、CSS 设计系统、浏览器调试、文档站和社区组件数量上天然更强。open-gpui 应避免追逐完整 Web
> 组件宇宙，优先互操作或参考 recipes，把差异化放在原生桌面高密度应用、文本、命令面板、dock、树表和长期运行体验。

### 主题与设计系统

#### `theme_token_model`

> shadcn/ui 的主题模型以 components.json、baseColor、iconLibrary、Tailwind 配置、CSS 变量、light/dark 变量和组件源码中的 class/variant
> 为核心；registry item 可携带 cssVars、css、style、base、theme、font 等信息。open-gpui 可借鉴这种 token-first 模型，但应把 token 设计为类型化
> schema，覆盖颜色、间距、圆角、字体、阴影、状态、密度、平台模式和 fallback。

#### `style_customization_boundary`

> shadcn/ui 的样式边界是：CLI 和 registry 提供默认文件，theme recipe 提供 CSS 变量，组件源码和 variants 提供局部样式，最终应用拥有并可修改全部代码。open-gpui 应明确为 core
> primitive 不绑定视觉风格，官方 theme recipe 提供默认视觉，组件 props 处理有限变体，用户源码负责最终 override，app adapter 处理平台差异。

### 组件表面

#### `component_coverage`

> 组件覆盖很广：form 和 input、layout 和 navigation、overlay 和 dialog、feedback 和 status、display 和 media、data
> table、chart、carousel、typography、sidebar、command palette、pagination、RTL、dark mode、forms 集成、monorepo 和多框架安装。

#### `must_have_for_open_gpui`

> open-gpui 必须优先补齐的是源码可拥有的组件 scaffold、机器可读 registry、基础 anatomy、主题 token、overlay
> primitive、表单基础控件、button/input/select/dialog/menu/tabs/tooltip/popover、docs gallery、示例编译门禁和 AI 可读
> metadata。这样才能形成可增长生态，而不是只提供一组零散 Rust widget。

#### `do_not_chase`

> 当前阶段不适合追 shadcn/ui 的全部 Web 能力：不要复刻 Tailwind class 体系、Next.js/RSC 安装矩阵、营销 blocks、页面模板、复杂 chart/carousel 生态、浏览器 CSS 细节、完整
> Figma 工作流和 Web-only 表单库。open-gpui 应先做桌面原生高价值组件和分发协议。

### 文档测试工具

#### `docs_gallery_model`

> shadcn/ui 把 docs、组件示例、registry schema、CLI、directory、llms.txt、MCP 和 skills 串成同一套分发体验，但不是所有页面都严格从同一个 schema 自动派生。open-gpui
> 应更进一步：registry item 成为 docs、gallery、examples、AI context、截图测试、可访问性测试和 scaffold 的共同事实源。

### 治理

#### `maintenance_cost`

> 维护成本中高。优点是 copy-to-own 降低中心化组件库对所有用例负责的压力，社区可以通过 registry 扩展；缺点是核心团队要维护
> CLI、schema、docs、gallery、examples、默认主题、迁移说明和兼容测试。对 Rust/native UI 来说，初期成本更高，因为需要同时建设 primitive、可访问性、主题、分发和验证基础设施。

#### `risks`

> 主要风险是把 Web/Tailwind/React 的隐含假设带进 native UI，导致架构错位；复制源码造成生态碎片化和升级漂移；第三方 registry 质量参差；AI 生成组件如果缺少 contract tests
> 会不可验证；过度追求组件数量会稀释 GPUI 的性能优势；registry schema 若设计过窄，后续会难以表达状态、可访问性和性能契约。

#### `open_gpui_relevance`

> 建议为 trial：采用 shadcn/ui 的复制后自有、registry、CLI、docs 和 AI-friendly 思路做 open-gpui 试点，但不要采用其 Web 运行时和 Tailwind 结构。直接设计含义是先定义
> gpui registry schema、components manifest、theme token schema、xtask/gpui CLI add/diff/verify、docs gallery 生成，以及 8 到 12
> 个核心组件的 end-to-end 分发闭环。

### 不确定字段（已跳过）

- `accessibility_model`
- `copy_modify_verify_loop`
- `design_token_pipeline`
- `diagnostics_and_failure_quality`
- `interaction_state_machines`
- `performance_model`
- `positioning_and_collision_model`
- `registry_viability`
- `rust_distribution_fit`
- `state_ownership_model`
- `testing_strategy`
- `third_party_ecosystem_path`
- `versioning_and_breakage`

## <a id="radix-ui-primitives"></a>2. Radix UI Primitives

- 结果文件：`Radix_UI_Primitives.json`
- 调研类别：`headless_accessible_primitives`
- 纳入原因：
  无样式、可组合、a11y/focus/keyboard primitive 的标杆；适合作为 open-gpui 行为层边界参考。
- 参考来源：
  - https://www.radix-ui.com/primitives

### 定位

#### `positioning`

> Radix UI Primitives 的生态定位是面向 React 与 Web DOM 的无样式、低层、可组合 headless primitive
> 库，重点承担可访问性、焦点管理、键盘交互、弹层定位、状态协调等行为层职责，而不是提供完整视觉主题或应用框架。

#### `target_users`

主要服务设计系统作者、React Web 应用团队、需要渐进采用可访问组件的产品团队，以及希望在自己的视觉体系上复用成熟交互行为的组件库维护者。

#### `primary_value_proposition`

> 核心价值是把常见复杂 UI 模式中最容易出错的行为和可访问性细节沉入 primitive，同时把样式、结构封装和高层 API 留给使用者。它与 open-gpui 的匹配点不是 React API 本身，而是行为层边界、组件
> anatomy、受控/非受控状态、overlay/focus/keyboard contract 的设计方法。

### 分发与生态

#### `distribution_model`

> Radix 采用 npm package dependency 分发：推荐安装可 tree-shake 的聚合包 `radix-ui` 并按需导入，也允许安装单个 `@radix-ui/react-*` primitive 包。它不是
> copy-to-own 组件注册表，也不是 CLI add/scaffold 模式；用户通过依赖包升级获得修复，通过 wrapper 和 `asChild` 组合出自己的设计系统 API。

#### `source_ownership`

> 使用者默认不拥有本地组件源码，而是依赖 MIT 开源包；可以查看源码、fork、patch 或包一层自己的组件，但常规路径是依赖 Radix 的行为实现。升级成本主要来自包版本同步、React 版本兼容、行为变更和 wrapper
> 层适配；相比 copy-to-own，patch 成本更低但深度改行为需要 fork。

### AI 时代设计

#### `ai_friendliness`

> 较高。官方文档按 Introduction、Getting started、Accessibility、Styling、Animation、Composition、每个组件的 Anatomy、API
> Reference、Accessibility、Keyboard Interactions、Custom APIs 组织；组件 parts、props、data attributes、CSS variables、示例代码和
> TypeScript 类型都很利于 AI 检索、组合和改写。短板是缺少一个公开统一的机器可读组件 contract 文件。

#### `copy_modify_verify_loop`

> Radix 的常规循环是安装包、按 anatomy 组合 parts、用 `className`/data attributes/CSS variables/asChild 包装成团队自己的组件，再通过
> TypeScript、浏览器行为、键盘和屏幕阅读器验证。它没有把源码复制到项目内再修改的官方主路径；open-gpui 若面向 AI 生成，应该补上 scaffold 后的 contract test、截图/交互/a11y gate
> 和示例回归验证。

### API 与组合

#### `api_ergonomics`

> API 形态是声明式组合：`Root` 持有上下文，`Trigger` 触发交互，`Portal` 处理脱离层级渲染，`Content` 承载弹层，`Item`/`Indicator`/`Arrow`/`Close` 等 parts
> 暴露细粒度结构。所有渲染 DOM 的 parts 普遍支持 `asChild`，状态组件通常支持 `defaultOpen/open/onOpenChange` 或
> `defaultValue/value/onValueChange`，调用体验一致、可预测、适合设计系统封装。

#### `customization_model`

> 样式完全由使用者控制：Radix 不提供默认视觉样式，使用者可用任意 CSS 方案；状态通过 `data-state` 等 data attributes 暴露，弹层几何通过 CSS variables 暴露，结构和底层元素通过
> `asChild` 替换，事件、props、ref 可继续透传。行为层可通过受控 props、事件回调、modal/non-modal、dismiss/focus/positioning props 调整，但深层算法仍在库内。

#### `component_anatomy_model`

> 非常明确。复杂组件被拆成稳定 parts，例如 Dialog/Popover 的 `Root`、`Trigger`、`Portal`、`Content`、`Close`、`Arrow`，Dropdown Menu 的
> `Group`、`CheckboxItem`、`RadioGroup`、`Sub`、`SubTrigger`、`SubContent`，Select 的
> `Value`、`Icon`、`Viewport`、`ItemText`、`ItemIndicator`、滚动按钮等。这个模型适合 open-gpui 映射为行为实体、渲染 element、portal/overlay host 和可替换
> slot。

#### `state_ownership_model`

> 默认非受控，适合快速使用；需要与应用状态同步时提供受控 props 和 change 回调。内部负责行为 wiring、焦点移动、dismiss、键盘导航、typeahead、隐藏 input 等细节；外部可提升 open/value
> 状态但不必重写交互。open-gpui 可借鉴为“组件内部可自治、应用可接管关键状态、运行时 handle 只处理命令式焦点/测量/弹层”的分层。

### Headless 与行为

#### `headless_boundary`

> Radix 的 headless 边界较清楚：可访问性语义、键盘、焦点、状态、dismiss、layering、positioning 属于 primitive；视觉样式、布局外观和设计 token 属于用户。需要注意它仍绑定 React
> 与 DOM，且 overlay 几何和 DOM portal 是实现层的一部分。open-gpui 应抽象出 renderer-neutral 行为 contract，再由 GPUI element/AccessKit/overlay
> host 适配。

#### `accessibility_model`

> Radix 遵循 WAI-ARIA authoring practices，并处理 aria/role 属性、focus management、keyboard navigation、label/description
> 等常见难点；复杂组件文档列出键盘交互表，Dialog/Popover/Select/Dropdown 等处理焦点进入、返回、Esc 关闭、screen reader announcement。open-gpui 不能照搬 ARIA
> 属性，但应建立等价的 AccessKit 节点、label/value/action/relationship、焦点路径和键盘 contract。

#### `positioning_and_collision_model`

> Radix 对 overlay primitive 的抽象很成熟：Popover、Dropdown Menu、Select 等支持
> side、align、sideOffset、alignOffset、avoidCollisions、collisionBoundary、collisionPadding、arrowPadding、sticky、hideWhenDetached、Arrow、modal/non-
> modal、outside interaction、Esc、focus return，并通过 `data-side`/`data-align` 和几何 CSS variables 暴露运行时碰撞结果。open-gpui
> 应优先把这些概念转成纯 geometry contract，而不是绑定 CSS 变量。

#### `interaction_state_machines`

> Radix 没有把有限状态机作为公开 API 暴露，但通过 controlled/uncontrolled props、`data-state`、事件回调、键盘交互表、focus/dismiss 回调形成了可测试的等价 contract。对
> Rust 原生实现来说，应进一步显式化为每个 primitive 的状态图和事件表，尤其是 menu/select/dialog/tabs/combobox/tree/table 这类键盘路径复杂组件。

### 渲染与性能

#### `rendering_model`

> Web React DOM 模型：组件渲染 DOM 元素，使用 React context、refs、portal、事件系统和 CSS。它不是 native retained UI、immediate mode、自绘或 GPU scene
> 框架。

#### `native_advantage`

> open-gpui 的 native 优势应放在 Radix 不覆盖的区域：大文本/代码编辑、大树/表格/列表、低延迟输入、GPU 合成、窗口级 overlay、原生菜单/拖拽/多窗口、跨平台 AccessKit 集成和非 DOM
> 布局性能。Radix 可作为行为正确性的标杆，但性能差异化应来自 GPUI 渲染与数据结构，而不是复刻 React DOM。

#### `web_ecosystem_advantage`

> Web 生态天然更强的是 ARIA/浏览器/屏幕阅读器成熟链路、CSS 动画和选择器、npm 包分发、React 设计系统、Storybook/Chromatic、现成文档和社区 recipes。open-gpui 不应在早期硬追完整
> Web 组件生态，而应优先做 native 场景更强的 primitive，并为 Web 思维迁移提供命名和 contract 上的互操作桥梁。

### 主题与设计系统

#### `theme_token_model`

> Radix Primitives 本身基本不提供视觉 theme token；它只暴露行为状态 data attributes 和部分几何 CSS variables。主题、颜色、尺寸、圆角、阴影、变体由用户设计系统或 Radix
> Themes 等其他层负责。open-gpui 应把 primitive 行为和 theme token 明确拆开，避免 headless 层承担视觉决策。

#### `design_token_pipeline`

> Radix Primitives 未体现 DTCG、Style Dictionary 或 Tailwind-like token transform 管线；它是 token pipeline 的消费者或底层行为依赖，而不是 token
> 编译系统。对 open-gpui 来说，design token pipeline 应作为独立层存在，向 primitive wrapper 提供状态/尺寸/颜色 recipe，而不污染行为 contract。

#### `style_customization_boundary`

> 样式边界几乎完全在用户侧：framework 负责行为和必要语义，用户源码或设计系统 wrapper 负责 class、CSS、动画、尺寸、颜色、布局和视觉变体；component prop 主要用于行为和结构，`data-state`
> 等属性连接行为状态与样式。open-gpui 可改成 theme recipe + component style hook + app adapter 三层，而核心 primitive 只输出状态和几何信息。

### 组件表面

#### `component_coverage`

> 覆盖基础控件、表单控件、overlay、navigation、feedback 和工具类：Accordion、Alert Dialog、Aspect Ratio、Avatar、Checkbox、Collapsible、Context
> Menu、Dialog、Dropdown Menu、Form、Hover Card、Label、Menubar、Navigation Menu、OTP/Password Toggle 预览组件、Popover、Progress、Radio
> Group、Scroll Area、Select、Separator、Slider、Switch、Tabs、Toast、Toggle、Toggle Group、Toolbar、Tooltip，以及 Accessible
> Icon、Direction Provider、Portal、Slot、Visually Hidden 等 utilities。

#### `must_have_for_open_gpui`

> 必须借鉴的是组件 anatomy、受控/非受控模型、键盘表、焦点管理、dismiss/layering、overlay collision、可访问性 contract、state-to-style 暴露和自定义高层 API 的
> wrapper 模式。open-gpui 若要成为通用 UI 框架，至少应先补齐
> Dialog、Popover、Menu、Tooltip、Select、Tabs、Checkbox、Radio、Switch、Slider、Toast、Scroll Area、Toolbar 等行为 primitive。

#### `do_not_chase`

> 当前阶段不应追逐 React DOM 专属细节、CSS selector/data attribute 的逐字复刻、隐藏 input 表单兼容、完整 npm 风格包生态、所有预览组件、Web 动画库适配和纯 Web ARIA
> 属性复制。open-gpui 更应追逐等价行为语义、AccessKit 映射和 native 性能优势。

### 治理

#### `versioning_and_breakage`

> Radix 通过 npm 包版本、变更日志和 SemVer 语义维护兼容性；官方建议单独安装多个 primitive 时一起更新以避免共享依赖重复和包体膨胀。近期 release 还使用 `unstable_` 前缀暴露新组合
> parts，说明其对破坏性 API 保持谨慎。open-gpui 应学习这种稳定 API 与实验 API 分层，并为行为 contract 变更提供 migration guide。

#### `maintenance_cost`

> 维护成本很高：每个 primitive 都要同时维护行为、焦点、键盘、a11y、定位、React 兼容、浏览器差异、文档、示例和测试。对 open-gpui 来说，若直接追完整 Radix 覆盖面会消耗巨大；更合理的是先建立少数核心
> primitive 的深 contract 和测试基座，再逐步扩展组件面。

#### `risks`

> 主要风险是把 Web DOM/ARIA/CSS 细节机械搬到 native Rust，导致抽象错位；组件 parts 过细会提高 API 学习成本；overlay/focus/layering 算法复杂且跨平台差异大；没有机器可读
> contract 时 AI 生成 wrapper 难验证；若主题、组件、registry、示例同时推进，容易形成碎片化生态。

#### `open_gpui_relevance`

> 建议定位为 reference-only，并对 overlay、focus、keyboard、component anatomy 做 trial。不要采用 React API 或 DOM 绑定实现，但应把 Radix
> 作为行为层边界标杆：先定义 renderer-neutral primitive contract，再分别落到 GPUI element、AccessKit、theme recipe、docs/gallery 和测试工具链。直接设计含义是
> open-gpui 的通用 UI 框架应优先成为“native headless primitive + recipe/gallery + contract tests”，而不是先做完整视觉组件库。

### 不确定字段（已跳过）

- `diagnostics_and_failure_quality`
- `docs_gallery_model`
- `machine_readable_contracts`
- `performance_model`
- `registry_viability`
- `rust_distribution_fit`
- `testing_strategy`
- `third_party_ecosystem_path`

## <a id="floating-ui"></a>3. Floating UI

- 结果文件：`Floating_UI.json`
- 调研类别：`positioning_collision_primitives`
- 纳入原因：
  > tooltip、popover、menu、dropdown 的定位、collision、flip、shift、size、arrow、safe polygon 行为高度可迁移；应研究是否抽象成 GPUI overlay behavior
  > kernel。
- 参考来源：
  - https://floating-ui.com/docs/computeposition
  - https://floating-ui.com/docs/middleware
  - https://floating-ui.com/docs/useinteractions

### 定位

#### `positioning`

> Floating UI 是 overlay 定位与交互行为 primitive，不是完整组件库。它的核心定位是为 tooltip、popover、menu、dropdown 等浮层提供 reference/floating
> 几何计算、collision 处理、middleware 组合和 React 交互 hooks，可视为 headless positioning infrastructure。

#### `target_users`

> 主要服务 Web 应用开发者、设计系统作者、headless 组件库作者，以及需要跨平台定位抽象的框架作者。对 open-gpui 来说，最值得服务的是通用 UI 框架维护者和桌面产品团队，而不是直接复用其 DOM/React 消费者
> API。

#### `primary_value_proposition`

> 核心价值是把浮层定位从组件实现中剥离出来：输入 reference/floating 的矩形、placement、strategy 和 middleware，输出最终 x/y、placement、strategy 与
> middlewareData。它与 open-gpui 的匹配度很高，因为 GPUI 也需要统一的 overlay 行为内核来避免每个 tooltip/menu/popover 重复实现
> flip、shift、size、arrow、dismiss 等细节。

### 分发与生态

#### `distribution_model`

> Floating UI 以 npm package dependency 分发，按能力拆成核心定位、DOM platform、React bindings 等包，用户通过导入
> computePosition、middleware、useFloating、useInteractions 等 API 组合能力。它不是 copy-to-own、registry 或 CLI add 模式；生态扩展主要通过包依赖和自定义
> middleware 实现。

#### `source_ownership`

> 用户通常依赖库 API 而不是拥有生成源码；好处是升级集中、bugfix 可快速接收，代价是 API 变更和行为细节受上游约束。对 open-gpui 更合适的方式是借鉴算法边界与 contract，在 Rust crate 内实现自有
> overlay kernel，让应用代码拥有稳定 typed API，而不是复制 JS 源码或绑定外部运行时。

### AI 时代设计

#### `ai_friendliness`

> 非常适合 AI 学习和迁移，因为它把问题拆成稳定词汇：reference、floating、placement、strategy、middleware、rects、middlewareData、platform、interaction
> hooks。对 AI 生成 open-gpui 组件尤其有价值：让模型先选择 overlay behavior recipe，再填 GPUI Element/Entity 渲染层，减少把定位逻辑散落在组件里的概率。

#### `machine_readable_contracts`

> Floating UI 提供强类型 TypeScript API 和清晰的 middleware return contract，但不是独立 JSON/YAML schema 或 registry manifest。open-gpui
> 可以把它升级为机器可读
> contract：OverlaySpec、Placement、CollisionPolicy、Boundary、MiddlewarePipeline、InteractionPolicy、A11yRole、FocusPolicy
> 都应可序列化，以便驱动 docs、gallery、scaffold 和交互测试。

#### `copy_modify_verify_loop`

> Floating UI 的复制修改循环主要是导入函数、组装 middleware、在浏览器中验证视觉和交互。open-gpui 应改为 recipe + contract 流程：复制一个 Tooltip/Menu/Popover
> recipe 后，开发者或 AI 修改 collision、delay、dismiss、focus return、safe polygon 等参数，再用 unit geometry tests、interaction
> tests、screenshot tests 和 a11y metadata tests 验证。

### API 与组合

#### `api_ergonomics`

> API 的精华是小核心加可组合管线：computePosition(reference, floating, options) 负责几何结果，middleware 数组负责逐步修正坐标或提供数据，useInteractions 把
> useHover/useClick/useFocus/useDismiss/useRole 等交互 hooks 合并成 prop getter。迁移到 GPUI 时，应保留“声明 placement +
> middleware/policy”的体验，但用 Rust builder、typed enum、trait object 或静态 enum pipeline 替代 JS 数组和 React prop getter。

#### `customization_model`

> 定位行为通过 middleware 顺序、middleware options、platform adapter、自定义 middleware 扩展；交互行为通过 hooks 参数控制
> hover/click/focus/dismiss/role/list navigation/typeahead 等。它不负责视觉主题。open-gpui 应把自定义分三层：行为参数和 middleware 改
> geometry，interaction policy 改打开/关闭/hover intent/focus，Element/Style/theme token 改外观。

#### `component_anatomy_model`

> Floating UI 不提供完整视觉组件 anatomy，但隐含了 root/reference/floating/arrow/item/list/portal/focus-manager 等 parts。对 GPUI 可映射为
> OverlayRoot、OverlayAnchor、OverlaySurface、OverlayArrow、OverlayItem、OverlayPortal、FocusScope，并让
> Tooltip、Popover、Menu、Dropdown 共享同一 anatomy vocabulary。

#### `state_ownership_model`

> React 侧通常由 useFloating 和 interactions 管理 refs、open state、context、floatingStyles、prop getters，也支持外部传入 open/onOpenChange
> 形成受控状态。对 open-gpui 应采用 application-owned 或 component-owned 二选一的显式模型：OverlayState 保存
> open、placement、active_index、focus_origin 等，OverlayRuntime 只负责测量、定位、订阅窗口变化和派发 dismiss/focus return。

### Headless 与行为

#### `headless_boundary`

> 边界划分清晰：computePosition 和 middleware 是 headless 几何层，platform 适配 DOM/React Native 等环境，React hooks 是交互绑定层，样式和渲染由用户负责。open-
> gpui 应照此分层：geometry kernel 不依赖 Element；interaction state machine 不依赖主题；render adapter 只消费 x/y/placement/arrow data/focus
> metadata。

#### `accessibility_model`

> Floating UI 的 React 层通过 useRole、useFocus、useDismiss、FloatingFocusManager、list navigation 等能力覆盖常见 ARIA/focus/keyboard
> 需求，但核心 computePosition 不处理无障碍语义。open-gpui 不能直接复制 ARIA，需要映射到 AccessKit 或 GPUI 自身 accessibility
> metadata：role、label、relationship、focus scope、escape dismiss、outside press、focus return 都应成为 overlay contract 的一部分。

#### `positioning_and_collision_model`

> 应重点采纳。核心模型是：先根据 placement 生成基础坐标，再按顺序运行 middleware；offset 调整距离，shift 保持在 clipping boundary 内，flip 在溢出时换边，autoPlacement
> 选择空间最多方向，size 根据可用空间改尺寸，arrow 提供箭头定位数据，hide 提供脱离 reference 时的隐藏数据，inline 支持多 rect reference。safe polygon 属于 hover
> 交互策略，可和 dismiss、focus return 一起放入 OverlayInteractionPolicy。

#### `interaction_state_machines`

> Floating UI 的 React hooks 是可组合交互 contract，但公开形态不是显式 finite state machine；状态通过 hook context、open/onOpenChange、event
> handler 合成和 list navigation 管理。open-gpui
> 应把它进一步显式化为可测试状态机：Closed、OpeningDelay、Open、HoverGrace、ClosingDelay、Dismissed、FocusRestoring，并把 pointer、keyboard、outside
> press、escape、scroll/resize 更新作为事件。

### 渲染与性能

#### `rendering_model`

> Floating UI 的默认落地是 DOM/Web/React：库计算位置，用户把 x/y 或 floatingStyles 应用到 DOM 节点；核心算法通过 platform adapter 可迁移到非 DOM 环境。

#### `native_advantage`

> open-gpui 的优势在于可以直接使用窗口、显示器、DPI、文本布局、滚动容器和 scene graph 信息，避免 DOM layout/reflow 与 portal clipping
> 的历史包袱。复杂桌面场景如多窗口、多显示器、嵌套滚动区域、命令面板、上下文菜单、代码编辑器 tooltip，应比 WebView 更容易获得一致的坐标和焦点行为。

#### `web_ecosystem_advantage`

> Web 生态在 ARIA 经验、浏览器兼容、成熟 hooks、真实用户验证和第三方组件集成上明显更强。open-gpui 不应追求兼容 Floating UI 的 React API，而应保持术语和行为对齐，必要时在文档中给出 Web 到
> GPUI 的迁移表，降低开发者和 AI 的认知成本。

### 主题与设计系统

#### `theme_token_model`

> Floating UI 基本不提供 theme token；它只输出定位和交互数据，视觉样式由消费者决定。open-gpui 应保持这个边界：overlay kernel 不拥有颜色、阴影、圆角、动画 token，只提供
> placement、side、alignment、arrow offset、available size 等状态，主题层根据这些状态选择样式。

#### `style_customization_boundary`

> 样式边界非常清楚：库负责行为，用户负责 DOM 结构和样式。open-gpui 应延续该边界：framework 提供 OverlayResult 和 interaction metadata，theme recipe
> 提供默认阴影、边框、动画，component prop 允许选择尺寸/variant，用户源码可替换 surface/arrow/item 渲染，app adapter 负责平台菜单或窗口级策略。

### 组件表面

#### `component_coverage`

> 覆盖重点是 overlay 族能力，而非全套组件：tooltip、popover、menu、select、combobox、dialog/focus、list
> navigation、typeahead、dismiss、role、hover/click/focus 等行为 primitive。基础表单、数据表格、导航、应用壳、富文本等不在其覆盖范围。

#### `must_have_for_open_gpui`

> 必须补齐。一个通用原生 UI 框架如果没有统一 overlay positioning/collision/interaction kernel，后续 tooltip、dropdown、context menu、command
> palette、autocomplete、color picker 都会产生重复且不一致的实现。建议优先实现 geometry + middleware + overlay scheduler，再逐步接入
> Tooltip、Popover、Menu 三个垂直切片。

#### `do_not_chase`

> 当前阶段不要追 React hooks API、DOM prop getter、ARIA 属性名兼容、CSS position strategy 细节、npm 式包拆分，也不要过早复刻完整 Floating UI React
> 组件体验。open-gpui 应追行为语义和测试 contract，而不是追 Web 运行时形态。

### 治理

#### `versioning_and_breakage`

> 包依赖模式要求上游 SemVer 控制 breaking change，用户升级时主要承担 API 和行为变化风险。open-gpui 若把 overlay kernel 作为基础
> crate，必须更保守：Placement/CollisionPolicy/MiddlewareData 等 public contract 一旦稳定就很难改；recipe 层可以更快迭代，核心几何层应提供 migration guide
> 和兼容测试。

#### `maintenance_cost`

> 实现一个 Floating UI 级别的行为内核维护成本中高：几何和 collision 本身可控，但真正昂贵的是平台测量、滚动/resize 更新、焦点管理、键盘交互、多窗口/DPI、a11y metadata、测试矩阵和长期 API
> 稳定性。收益也高，因为它能被所有 overlay 组件复用，避免后续组件各自承担隐藏成本。

#### `risks`

> 主要风险是过度复刻 Web：把 React hook、DOM portal、CSS strategy 原样搬进 native 会稀释 GPUI 优势。第二个风险是 middleware 过度动态导致 Rust API
> 难以静态验证。第三个风险是只做定位不做 interaction/focus/a11y，最终仍然无法支撑 menu/select/popover。第四个风险是缺少 diagnostics 和 contract tests，AI
> 生成组件会看似能跑但边界行为不一致。

#### `open_gpui_relevance`

> adopt：采纳 Floating UI 的设计内核和术语，但不要直接移植实现。open-gpui 应设计一个原生 OverlayBehaviorKernel：GeometryPlatform
> 负责测量，compute_overlay_position 负责 placement 和 middleware，OverlayInteractionPolicy 负责 hover/click/focus/dismiss/safe
> polygon，FocusPolicy 负责 focus trap/return，Diagnostics 负责可测试和 AI 修复。第一批落地目标建议是 Tooltip、Popover、Menu 共用同一套 contract。

### 不确定字段（已跳过）

- `design_token_pipeline`
- `diagnostics_and_failure_quality`
- `docs_gallery_model`
- `performance_model`
- `registry_viability`
- `rust_distribution_fit`
- `testing_strategy`
- `third_party_ecosystem_path`

## <a id="floating-ui-rust-crates"></a>4. Floating UI Rust crates

- 结果文件：`Floating_UI_Rust_crates.json`
- 调研类别：`native_port_candidate`
- 纳入原因：
  调研 floating-ui-core/dom 的 Rust 生态状态，判断是借鉴算法、直接依赖、fork，还是重写 GPUI-native positioning。
- 参考来源：
  - https://github.com/floating-ui/floating-ui
  - https://crates.io/search?q=floating-ui

### 定位

#### `positioning`

> Floating UI Rust 生态的核心定位不是完整组件库，而是对原版 Floating UI 的 Rust/Web 移植：`floating-ui-core` 提供平台无关定位算法，`floating-ui-dom` 提供
> DOM/web-sys 适配，另有 Leptos、Yew、Dioxus 适配层。对 open-gpui 来说，它是 overlay positioning 与 collision 策略参考，而不是直接可用的 GPUI 组件体系。

#### `target_users`

> 主要服务 Rust/WASM/Web 前端开发者、RustForWeb 生态维护者，以及需要在 Leptos、Yew、Dioxus 或裸 DOM/web-sys 中复用 Floating UI 定位能力的库作者；不是面向 GPUI、自绘桌面
> UI 或原生窗口系统的第一目标用户。

#### `primary_value_proposition`

> 核心价值是把 tooltip、popover、dropdown、menu 等浮层定位抽象为 `compute_position + platform + middleware`：core 算法负责
> placement、flip、shift、size、arrow、overflow 等计算，平台层负责测量和裁剪边界。它与 open-gpui 的匹配点是算法和合约设计，直接匹配度低于“重写 GPUI-native positioning”。

### 分发与生态

#### `distribution_model`

> Rust 侧以 Cargo/crates.io package dependency 分发：`floating-ui-core`、`floating-ui-utils`、`floating-ui-dom`、`floating-ui-
> leptos`、`floating-ui-yew`、`floating-ui-dioxus` 等独立 crate。crates.io 查询显示这些 RustForWeb crate 最新稳定发布为 0.6.0，GitHub main
> workspace 已标到 0.7.0；另有名称精确匹配的 `floating-ui` crate 仅 0.0.1、描述为 Rust bindings，生态权重较低。它没有 shadcn 式 copy-to-own registry，也没有
> GPUI recipe registry。

#### `source_ownership`

> 作为 MIT 许可的 crate，用户可以依赖、fork 或借鉴源码；但直接依赖会把 open-gpui 的核心 overlay API 绑定到外部 SemVer、泛型 Platform trait、serde_json
> middleware_data 和 RustForWeb 发布节奏。fork 能获得源码控制权，但需要长期跟随原版 Floating UI 算法与 RustForWeb 移植差异；更合理的是把算法概念、测试思路和 Platform
> 合约转译成 GPUI 自有类型。

#### `rust_distribution_fit`

> 与 Rust 分发模型的适配度中等偏高：crate 拆分清楚，Cargo 能直接依赖，MIT 许可友好，RustForWeb workspace 使用 edition 2024，crate 粒度与 core/dom/framework
> adapter 分层一致。问题是 DOM adapter 依赖 web-sys、ResizeObserver、VisualViewport 等浏览器类型，不能直接进入 GPUI native；core 虽平台无关，但需要为 GPUI
> 编写测量、clipping、viewport、RTL、scale 等 Platform 实现。

#### `third_party_ecosystem_path`

> 第三方路径主要是 Cargo crate 和 RustForWeb 仓库贡献：框架适配通过单独 crate 进入生态，示例与测试放在各框架目录中。对 open-gpui 更可取的路径是把 overlay 合约、golden
> tests、gallery story 和 recipe 放入本仓库，然后允许第三方组件依赖 open-gpui 的 positioning contract，而不是直接要求第三方适配 Floating UI Rust crate。

### AI 时代设计

#### `ai_friendliness`

> AI 友好度较高：原版 Floating UI 和 Rust 移植都围绕清晰概念建模，包括 placement、strategy、platform、middleware、rect、overflow、reset、middleware
> data。AI 能较容易检索和转译算法。但 DOM/Web 术语较强，AI 若直接迁移到 GPUI 容易错误引入 viewport、offset parent、document element 等浏览器假设。

#### `machine_readable_contracts`

> 有强类型 Rust API，但不是 registry manifest 或 schema 驱动系统。`Platform`
> trait、`ComputePositionConfig`、`Middleware`、`Placement`、`Strategy` 等可作为机器可读的类型契约；middleware data 使用
> serde_json，适合扩展但弱化了静态可验证性。open-gpui 应把这类合约收敛为 GPUI 原生 `Rect/UiPx/WindowBounds/OverlayPolicy` 类型，并尽量避免 JSON 作为核心中间态。

#### `copy_modify_verify_loop`

> 直接 copy 修改外部 crate 不适合长期维护；更好的循环是：先用 Floating UI 官方/RustForWeb 行为作为参考样例，抽取 placement、flip、shift、size、arrow、overflow 的
> golden cases，再在 open-gpui 内实现 GPUI-native resolver，通过单元测试、overlay gallery、视觉回归和交互测试验证。若短期 trial，可只在测试或实验 feature 中依赖
> `floating-ui-core` 对比输出。

### API 与组合

#### `api_ergonomics`

> API 核心形态是函数式 `compute_position(reference, floating, config)` 加 builder-like config 和 middleware vector；扩展点是 Platform
> trait 与 Middleware trait。对算法库来说清晰，对 GPUI 组件作者则偏底层。open-gpui 更适合暴露
> `OverlayPlacementPolicy`、`resolve_overlay_position`、`collision_strategy`、`arrow_policy` 这类更贴近组件状态的 API，再在内部借鉴 middleware
> 管线。

#### `customization_model`

> Floating UI 的定制主要通过 middleware 顺序与 options 实现，例如 offset、flip、shift、size、arrow、auto
> placement、inline、hide。样式、主题、结构、子组件不在它的职责范围内。open-gpui 可借鉴这一点：定位策略应是行为 contract，不应和主题 token、渲染节点、组件 anatomy 强耦合。

#### `component_anatomy_model`

> 它本身不提供 root/trigger/content/item/indicator 组件 anatomy，只处理 reference 与 floating element 的几何关系。对 open-gpui 的启发是把
> trigger/content/portal/arrow 的几何测量边界定义清楚：组件 anatomy 留在 `ui_components`，定位求解器留在 `ui_core` 或专门的 overlay geometry 模块。

#### `state_ownership_model`

> Floating UI core 基本无 UI 状态所有权，只接收当前元素矩形、placement、strategy、middleware，并返回坐标和最终 placement。open-gpui
> 应保持类似分层：open/closed、dismiss、focus restore、controlled/uncontrolled 仍由 overlay state machine 管；positioning resolver
> 只处理输入几何和策略，返回 resolved rect、placement、arrow offset、overflow diagnostics。

### Headless 与行为

#### `headless_boundary`

> headless 边界清楚：core 是平台无关几何算法，DOM 和框架 crate 是 render/platform adapter，a11y、样式和交互状态不是 core 职责。open-gpui 应沿用这个边界，但把 DOM
> 平台概念替换为 GPUI window、deferred layer、anchor bounds、safe bounds、scale factor、viewport/inset 等原生概念。

#### `accessibility_model`

> Floating UI Rust crates 的主要职责不是 a11y；role、focus、keyboard、screen reader 需要由上层组件框架处理。open-gpui 已有 overlay kind、outside
> press、focus restore、initial focus 等 renderer-neutral policy，后续应把 positioning 与 AccessKit/role/focus 流程并列验证，而不是期望
> Floating UI 提供完整 a11y 模型。

#### `positioning_and_collision_model`

> 这是最值得借鉴的部分：Floating UI core 覆盖 placement、strategy、offset、flip、shift、size、arrow、hide、inline、auto placement、overflow
> detection 和 middleware reset。open-gpui 当前更像有 side/alignment/offset/safe_bounds 与 GPUI anchored 映射，应补一个 GPUI-native
> collision resolver：输入 anchor rect、content size、window safe rect、preferred side/alignment、offset、padding、arrow size，输出
> final rect、final placement、arrow position、clamped size、overflow diagnostics。

#### `interaction_state_machines`

> Floating UI core 不负责 menu/select/dialog/tabs 等有限状态机；它只在定位 middleware 内有 reset 循环避免无限重算。open-gpui 不能把它当作交互状态机来源，应继续用自身的
> overlay disclosure、menu roving focus、dismiss/focus policy 等 contract，并把定位变化作为状态机的一个可测试副作用。

### 渲染与性能

#### `rendering_model`

> 原版是 DOM/WebView 生态，RustForWeb 的 DOM crate 通过 web-sys 与浏览器布局测量交互；core crate 是纯几何计算。open-gpui 是 native retained/GPU scene
> 与 deferred layer 模型，最适合只吸收 core 几何模型，不吸收 DOM rendering model。

#### `native_advantage`

> GPUI native 的优势在于可以直接使用窗口安全区域、真实设备像素/缩放、GPU scene、deferred layer、应用级焦点树和自有布局数据，避免 DOM offset parent、scroll
> ancestor、ResizeObserver 的复杂性。对 tooltip/menu/popover，GPUI 可以把测量与渲染管线绑定得更紧，从而获得更可预测的 collision 和更好的测试诊断。

#### `web_ecosystem_advantage`

> Web/Tauri/Electron 生态在成熟度、浏览器 edge case、跨框架 adapter、文档、社区问题覆盖和可访问性实践上明显更强；Floating UI 已长期服务 Web overlay 场景。open-gpui
> 不应追求完整复刻 Web 的 offset parent、layout viewport、visual viewport、inline DOM range 等全部语义，只应在需要互操作或行为对齐时参考。

### 主题与设计系统

#### `theme_token_model`

> Floating UI 基本不处理主题 token；它只提供几何与 middleware 数据。open-gpui 的 theme token 应继续由 `ui_core/ui_components` 的
> ThemeTokens、recipes 和组件状态负责，positioning contract 只需要接受与布局相关的数值 token，例如 spacing、offset、arrow size、safe margin、collision
> padding。

#### `design_token_pipeline`

> 该生态不提供 DTCG、Style Dictionary 或 Tailwind-like token pipeline。对 open-gpui 的启发是不要把定位算法塞进 token pipeline；可以让 token pipeline
> 产出 overlay spacing、radius、shadow、arrow size 等设计值，再由 GPUI-native resolver 消费。

#### `style_customization_boundary`

> 样式边界应保持在 open-gpui 组件/主题层，Floating UI 风格的定位层只返回坐标、尺寸和元数据。open-gpui 应避免让 positioning middleware 直接决定背景、边框、阴影或组件结构；这些应由
> theme recipe、component prop 或用户源码控制。

### 组件表面

#### `component_coverage`

覆盖的是 overlay 几何能力，不覆盖完整 UI 组件面：tooltip、popover、dropdown、menu 等只作为定位使用场景出现；没有按钮、表单、导航、数据展示、应用壳或富文本组件。

#### `must_have_for_open_gpui`

> 必须补齐的不是对 RustForWeb crate 的依赖，而是同等级的 overlay positioning 能力：flip、shift、collision padding、size constraint、arrow
> positioning、context-menu point anchor、submenu safe bounds、RTL/scale、诊断输出和 gallery
> contract。没有这些，popover/menu/select/combobox 在窗口边缘、多屏、缩放和嵌套菜单场景会不稳定。

#### `do_not_chase`

> 当前阶段不应追 DOM 专属能力：offset parent 兼容矩阵、VisualViewport、ShadowRoot、Range/inline DOM rect、ResizeObserver autoUpdate
> 的完整语义、React/Vue adapter parity、浏览器滚动祖先模型。也不必追求和 Floating UI middleware 名称百分百兼容；GPUI 应保留自己的术语和类型。

### 文档测试工具

#### `docs_gallery_model`

> Floating UI/RustForWeb 有官方文档、book、framework examples 和移植测试，适合学习示例组织方式。open-gpui 应把 overlay docs/gallery/story/AI
> examples 从同一组 behavior contracts 派生：每个 story 声明 anchor、content size、preferred placement、safe bounds、预期 final placement
> 与交互策略。

#### `testing_strategy`

> 建议建立三层测试：一是纯函数 geometry tests，覆盖 top/right/bottom/left、start/center/end、flip、shift、size、arrow、safe bounds、RTL、scale；二是组件
> interaction tests，覆盖 outside press、Escape、focus return、submenu hover path；三是 gallery/visual tests，覆盖窗口边缘、滚动、缩放、窄屏和多浮层
> z-order。可用 Floating UI 的官方测试思路作为参考，但断言应落在 GPUI 原生类型上。

#### `diagnostics_and_failure_quality`

> Floating UI 返回 middleware_data，但诊断面向库使用者而非 open-gpui 的 gallery/AI 修复。open-gpui 应输出更高质量的 failure：组件 id、anchor
> rect、content size、safe rect、preferred/final placement、触发的 collision 策略、被 clamp 的尺寸和建议修复方向。这样更适合 AI 自动定位失败原因。

### 治理

#### `risks`

> 主要风险是生态错位：把 Web/DOM 的复杂测量模型带入 GPUI，稀释 native 优势；直接依赖外部 core 后，open-gpui overlay API 被 middleware/dyn trait/serde_json
> 形态牵引；fork 后长期同步成本高；若只做最小定位，又会在菜单、子菜单、combobox、窗口边缘和缩放场景反复补丁化。

#### `open_gpui_relevance`

> 最终建议：reference-only，辅以小范围 trial。不要直接 adopt `floating-ui-dom`，也不建议 fork 整个 RustForWeb 项目；可以短期用 `floating-ui-core` 做实验或
> golden reference，但产品化应重写 GPUI-native positioning。直接设计含义是：在 `ui_core` 中定义平台无关但 GPUI 语义明确的 overlay geometry contract，在
> `ui_components` 中消费 resolved placement，并用 gallery/tests 固化 collision 行为。

### 不确定字段（已跳过）

- `maintenance_cost`
- `performance_model`
- `registry_viability`
- `versioning_and_breakage`

## <a id="react-aria-react-aria-components"></a>5. React Aria / React Aria Components

- 结果文件：`React_Aria_React_Aria_Components.json`
- 调研类别：`accessible_behavior_library`
- 纳入原因：
  Adobe 对跨浏览器 a11y、interaction hooks、unstyled components 的系统化实践；可借鉴 API 表达和状态/action metadata。
- 参考来源：
  - https://react-spectrum.adobe.com/react-aria/
  - https://react-spectrum.adobe.com/react-aria/components.html

### 定位

#### `positioning`

> React Aria / React Aria Components 的生态定位是 Adobe React Spectrum 体系中的可访问行为基础设施与无样式组件库：React Aria 提供跨浏览器、跨设备、跨辅助技术的交互
> hooks；React Stately 提供行为无关的状态管理；React Aria Components 在此基础上提供更直接的无样式组件 anatomy。它不是视觉主题库或桌面 shell，而是面向设计系统和应用组件的
> a11y/interaction primitive 层。

#### `target_users`

> 主要服务 React 应用开发者、设计系统作者、需要可靠无障碍体验的产品团队、组件库维护者，以及希望用无样式组件快速构建定制 UI 的前端团队。对 open-gpui
> 来说，最值得参考的用户画像是框架作者和桌面产品团队：他们需要把复杂行为、焦点、键盘、屏幕阅读器语义沉入基础层，而不是让每个应用重复实现。

#### `primary_value_proposition`

> 核心价值是把浏览器差异、ARIA authoring practices、键盘导航、焦点管理、国际化、选择模型、overlay 行为和复杂集合组件状态系统化封装，同时保持样式与结构的可定制性。与 open-gpui 的匹配点不是
> React/DOM API，而是行为层 contract、状态/action metadata、组件 anatomy、集合模型、测试矩阵和文档组织方式。

### 分发与生态

#### `distribution_model`

> 分发方式以 npm package dependency 为主，围绕 `react-aria`、`react-aria-components`、`react-stately` 等包提供
> hooks、无样式组件和状态逻辑。官方还提供文档站、示例、Tailwind 集成说明、AI 专用文档入口、starter kits、设计系统示例和 React Spectrum 高层实现。它不是 copy-to-own registry 或
> CLI add 模式；用户通过依赖包升级获得行为修复，通过 wrapper、CSS、render props 和 slot 组合自己的设计系统。

#### `source_ownership`

> 使用者默认不拥有本地组件源码，而是依赖 Apache-2.0 开源包；可以阅读源码、fork 或封装自己的组件层，但常规路径是把 React Aria Components 当成稳定行为依赖。升级成本主要来自 React 版本、组件
> API、行为细节、CSS/data attribute/slot 约定以及设计系统 wrapper 的适配。相比 shadcn 式复制源码，它更适合维护统一行为质量，但深度改内部行为时 fork 成本较高。

### AI 时代设计

#### `ai_friendliness`

> 较高。官方提供面向 AI 的文档入口，包括 `llms.txt`、`llms-full.txt`、组件文档 Markdown、Agent Skills 和 MCP server，降低模型检索和引用错误；组件文档普遍包含
> anatomy、示例、样式方式、props、事件、可访问性说明和键盘交互。对 open-gpui 的启示是：组件 contract、示例、测试和文档应从一开始就为 AI 检索与修改设计，而不只是给人读的页面。

#### `copy_modify_verify_loop`

> React Aria 的常规循环是安装包，选用 hook 或 component 形态，使用 CSS、Tailwind、render props、slot、context 和 wrapper 改样式与结构，再用
> TypeScript、浏览器交互、键盘、屏幕阅读器和官方测试建议验证。它不是源码复制后本地改内部实现的主路径。open-gpui 若面向 AI 生成，应提供 recipe scaffold 后的 contract
> test、interaction test、AccessKit snapshot、视觉截图和性能门禁，形成“复制/生成-修改-验证”的闭环。

### API 与组合

#### `api_ergonomics`

> API 有两层：低层 React Aria hooks 通常返回 props、state 连接和交互 metadata，需要开发者自己渲染 DOM；高层 React Aria Components 提供声明式无样式组件，例如
> Button、DialogTrigger、Popover、ListBox、Select、Table、Tree 等，并通过 render props 暴露状态，通过 slots 连接 label/description/error 等
> parts，通过受控/非受控 props 管理状态。整体体验比纯 hooks 更接近 headless component，但仍保留强 escape hatch。

#### `customization_model`

> 定制模型很完整：样式可用普通 CSS、CSS modules、Tailwind、CSS-in-JS 等；运行状态通过 data attributes、render props 和 className/style
> 函数暴露；结构可通过组合子组件、slots、context 和自定义 wrapper 调整；行为可通过 props、受控状态、selection mode、keyboard delegate、drag and drop、overlay
> placement、focus strategy 等参数控制。深层可访问性算法仍在库内，用户主要定制外观、结构与策略参数。

#### `component_anatomy_model`

> 复杂组件有清楚的 parts 和组合边界，例如
> DialogTrigger/Modal/Popover/Dialog/Heading、Select/Button/Label/ListBox/ListBoxItem、ComboBox/Input/Button/ListBox、Table/TableHeader/Column/TableBody/Row/Cell、Tabs/TabList/Tab/TabPanel、Form/FieldError/Text。React
> Aria Components 不一定使用 Radix 式 Root/Trigger/Content 命名，但同样强调 anatomy、slot 和集合子项。open-gpui 可映射为 Entity 持有状态，Element 渲染
> parts，AccessKit 输出语义节点，overlay host 处理 portal/浮层。

#### `state_ownership_model`

> React Aria 体系非常重视状态分层：React Stately 管理可复用状态；React Aria hooks 连接状态、DOM props 与无障碍行为；React Aria Components 支持默认非受控和受控用法，例如
> selectedKeys/defaultSelectedKeys、isOpen/defaultOpen、value/defaultValue、onSelectionChange/onOpenChange 等。对 open-gpui
> 的启示是把 renderer-neutral state、application-owned state、component-owned state 和 runtime handle 明确分开，避免把焦点、选择、滚动、overlay
> 测量混进视觉组件。

### Headless 与行为

#### `headless_boundary`

> 边界清晰但分层比 Radix 更系统：React Stately 负责状态，React Aria 负责行为和 a11y props，React Aria Components 负责无样式组件 anatomy，React Spectrum
> 负责 Adobe 视觉系统。样式、主题和布局由消费者负责；浏览器/DOM/ARIA 适配由库负责。open-gpui 应采用类似分层：状态 crate、交互/a11y contract、无样式 native primitive、主题
> recipe 和产品级组件分离。

#### `accessibility_model`

> 这是最核心参考。React Aria 覆盖 role、ARIA 属性、label/description/error 关联、keyboard navigation、focus management、focus visible、screen
> reader 差异、国际化、RTL、selection、drag and drop、overlay dismiss、focus trap/restore 等；官方强调组件在浏览器、设备和辅助技术组合中测试。open-gpui 不能复制
> ARIA 属性名，而应定义等价的 AccessKit 节点、role、name、value、action、relationship、focus scope、keyboard intent 和状态
> metadata，并建立屏幕阅读器行为测试或快照。

#### `positioning_and_collision_model`

> React Aria Components 对 overlay 族提供 Popover、Tooltip、Dialog、Modal、OverlayArrow、DialogTrigger、MenuTrigger、ComboBox 等组合，支持
> placement、offset、crossOffset、containerPadding、shouldFlip、boundaryElement、arrowBoundaryOffset、triggerRef、isNonModal、isKeyboardDismissDisabled
> 等策略，并处理滚动、dismiss、focus containment/restore 和语义关系。open-gpui 应把这些抽象成独立 overlay
> contract：anchor、surface、placement、collision、arrow、dismiss policy、modal policy、focus return 和 diagnostics。

#### `interaction_state_machines`

> 公开 API 不以显式 finite state machine 呈现，而是通过 React Stately state objects、collection/selection state、overlay trigger
> state、hover/focus/press hooks、drag/drop hooks、键盘表和受控事件形成等价 contract。对 Rust 原生实现，应进一步显式化为可测试状态机或事件表，尤其覆盖
> press、hover、focus-visible、selection、combobox、listbox、table/tree navigation、menu、dialog、popover 和 drag/drop。

### 渲染与性能

#### `rendering_model`

> Web React DOM 模型：hooks 和 components 最终渲染 DOM 元素，依赖 React state/context/ref/event、portal、CSS、ARIA 和浏览器焦点系统。它不是 native
> retained UI、immediate mode、自绘或 GPU scene 框架。

#### `performance_model`

> 性能模型主要服务 Web 组件：通过 hooks/组件拆分、集合模型、虚拟化、按需渲染、React 状态控制和浏览器原生能力处理复杂 UI。官方提供 Virtualizer、Table、GridList、Tree
> 等集合组件，覆盖大集合、表格、树、拖拽、排序、选择等场景；但底层仍受 DOM、React render、浏览器 layout 和 CSS 约束。open-gpui 可借鉴集合/selection contract，但性能差异化应来自
> native retained tree、增量布局、GPU 绘制、低延迟输入和滚动所有权。

#### `native_advantage`

> open-gpui 应在 React Aria 不天然占优的场景建立优势：大文本/代码编辑、大表格/大树/虚拟列表、复杂 docking、命令面板、上下文菜单、低延迟输入、窗口级 overlay、多显示器/DPI 坐标、GPU
> 合成、原生字体与文本布局、AccessKit 深集成。React Aria 的价值是行为正确性标杆，native GPUI 的价值应是更强渲染、测量和桌面交互一致性。

#### `web_ecosystem_advantage`

> Web 生态天然更强的是 ARIA 标准积累、浏览器和屏幕阅读器兼容经验、CSS/Tailwind 生态、React 设计系统、npm 分发、Storybook/Chromatic、真实用户覆盖和大量现成组件。open-gpui
> 不应早期追完整 Web 组件面和 CSS 能力，而应与 Web 术语保持可迁移性，优先做 native 桌面场景更强的 primitive，并在文档中提供 React Aria 到 GPUI 的概念映射。

### 主题与设计系统

#### `theme_token_model`

> React Aria Components 本身是无样式层，不内置完整视觉 token；它通过 data attributes、CSS 变量和 render props 暴露状态，允许用户用自己的设计 token、Tailwind 或
> CSS 体系实现视觉。React Spectrum 才是带 Adobe 设计语言的上层实现。open-gpui 应照此拆分：primitive 输出状态、语义和几何；theme recipe 消费这些 metadata；产品组件再绑定具体
> token、尺寸、动画和视觉变体。

#### `style_customization_boundary`

> 样式边界在用户或设计系统侧：framework 负责行为、状态、语义和必要结构；用户用 className、style、CSS 选择器、data attributes、render props、slot 和 wrapper 实现视觉；上层
> React Spectrum 或团队设计系统负责 token 与默认外观。open-gpui 可设计为 core primitive 不含视觉承诺，官方 recipe 提供默认主题，应用 adapter 可替换每个 part 的渲染与样式。

### 组件表面

#### `component_coverage`

> 覆盖面很广：基础控件、表单、按钮、复选框、单选、开关、滑块、文本字段、搜索字段、数字字段、日期/时间字段、选择器、ComboBox、ListBox、Menu、Popover、Dialog、Modal、Tooltip、Tabs、Breadcrumbs、Disclosure、Meter、ProgressBar、Table、GridList、Tree、Toolbar、TagGroup、Calendar、DatePicker、Color
> 组件、Drag and Drop、Virtualizer 等，还包括 usePress、useHover、useFocusRing、useKeyboard、useMove、useOverlay、useListData、selection 等
> hooks。

#### `must_have_for_open_gpui`

> 必须借鉴的是 a11y 行为系统化、状态层分离、collection/selection 模型、slot/anatomy、受控/非受控状态、press/focus/hover 统一事件、overlay
> focus/dismiss、键盘导航表、国际化/RTL 思维、AI 可读文档和测试矩阵。open-gpui 通用 UI 框架应优先补齐
> Button、Checkbox、Radio、Switch、TextField、Select、ComboBox、ListBox、Menu、Popover、Dialog、Tooltip、Tabs、Table/Tree 的行为
> primitive，而不是先追完整视觉套件。

#### `do_not_chase`

> 当前阶段不应追逐 React hook 形态、DOM props getter、ARIA 属性逐字复刻、浏览器表单隐藏 input、CSS/Tailwind 专属细节、完整 React Spectrum 视觉系统、Web
> 动画生态和所有长尾组件。open-gpui 更应追等价语义、AccessKit contract、native interaction state、overlay/collection 内核和性能敏感组件。

### 治理

#### `versioning_and_breakage`

> 包依赖模式下，兼容性主要由 npm 版本、React Spectrum 变更记录、TypeScript API 和文档迁移承担。React Aria 的 hooks、components 和 React Stately 分层意味着
> breaking change 可能同时影响行为、状态和组件 wrapper。open-gpui 应学习“低层 contract 稳定、实验能力隔离、迁移指南清楚”的治理方式；对 AccessKit node
> contract、SelectionKey、OverlayPlacement、FocusPolicy 等基础类型要谨慎承诺 SemVer。

#### `maintenance_cost`

> 维护成本很高，因为它不是简单组件库，而是跨浏览器、跨输入设备、跨辅助技术、跨语言方向、跨复杂组件模式的行为系统。对 open-gpui 来说，直接追完整覆盖面风险过大；更合理的是先建立
> Button/Press、Focus、Overlay、Selection/Collection、Form Field、Table/Tree 等少数基础内核，并要求每个内核有 contract、文档、示例、测试和 diagnostics。

#### `risks`

> 主要风险是把 Web/React/ARIA 细节机械搬到 native Rust，导致抽象错位；第二是无样式组件 API 过细，学习和维护成本高；第三是 a11y 行为如果没有真实辅助技术验证，容易只停留在 metadata 层；第四是
> collection/table/tree/virtualizer 一旦设计不稳，会成为后续所有复杂组件的技术债；第五是 AI 生成 wrapper 若缺少 contract tests，会产生看似可用但键盘、焦点、屏幕阅读器不一致的组件。

#### `open_gpui_relevance`

> 建议 reference-only + targeted trial：不要采用 React/DOM API，也不要复制完整组件面；应把 React Aria 作为 a11y、interaction、state/action metadata
> 和 AI 文档工程的主要标杆。直接设计含义是 open-gpui 应优先设计 renderer-neutral 的
> `AccessibleNode/Action/FocusPolicy/PressInteraction/SelectionModel/OverlayPolicy/CollectionState` contract，再用 GPUI
> Element、AccessKit、theme recipe、gallery 和测试工具链落地。第一批 trial 目标建议是
> Press/Button、FocusRing、Dialog/Popover、ListBox/Select、Table/Tree。

### 不确定字段（已跳过）

- `design_token_pipeline`
- `diagnostics_and_failure_quality`
- `docs_gallery_model`
- `machine_readable_contracts`
- `registry_viability`
- `rust_distribution_fit`
- `testing_strategy`
- `third_party_ecosystem_path`

## <a id="zag-js"></a>6. Zag.js

- 结果文件：`Zag_js.json`
- 调研类别：`state_machine_primitives`
- 纳入原因：
  以 framework-agnostic state machine 表达 UI 行为；对 Rust 中 renderer-neutral state contract 很有参考价值。
- 参考来源：
  - https://zagjs.com/

### 定位

#### `positioning`

> Zag.js 的生态定位是 framework-agnostic、headless、以有限状态机/状态图表达复杂 UI 行为的 primitive
> 工具集。它不是视觉组件库，也不是渲染框架；核心职责是把交互状态、事件、可访问性属性、焦点管理、键盘行为和组件 parts 契约沉入机器层，再通过 React、Vue、Solid、Svelte adapter 连接到具体渲染环境。

#### `target_users`

> 主要服务设计系统作者、跨框架组件库维护者、Web 应用开发者，以及希望把交互行为从 React/Vue/Svelte 等框架生命周期中抽离出来的框架作者。对 open-gpui 最有参考价值的对象是通用 UI primitive
> 维护者和需要 renderer-neutral state contract 的桌面产品团队。

#### `primary_value_proposition`

> 核心价值是“行为写一次，多框架消费”：组件交互由机器定义，`connect` 把机器状态映射为 parts props 和事件处理器，`normalizeProps` 处理框架差异。它与 open-gpui 的目标高度匹配，因为 GPUI
> 也需要把复杂 UI 的状态、可访问性、焦点、键盘和定位行为从具体 Element 渲染中分离出来。

### 分发与生态

#### `distribution_model`

> Zag.js 采用 npm package dependency 分发：每个组件机器是单独的 `@zag-js/*` 包，例如 `@zag-js/menu`、`@zag-js/popover`、`@zag-js/tooltip`，框架
> adapter 另装 `@zag-js/react`、`@zag-js/vue`、`@zag-js/solid`、`@zag-js/svelte`。官方文档显示组件页会链接 npm、源码和 Logic Visualizer，并标出版本；不是
> copy-to-own、CLI add、源码 registry 或视觉模板分发。

#### `source_ownership`

> 使用者默认不拥有本地组件源码，而是依赖 MIT 开源机器包；可以 fork、patch 或参考源码，但常规路径是升级 npm 依赖。由于每个组件机器独立发包，升级可以按机器粒度进行，但也带来批量升级和跨包版本对齐成本。相对源码复制，用户
> patch 行为的成本更高；相对完整组件库，行为修复可以集中从上游获得。

### AI 时代设计

#### `ai_friendliness`

> 很高。Zag.js 官方提供 `llms.txt`、`llms-full.txt`、按框架拆分的 LLM 文档入口；每个组件文档通常包含功能说明、安装、Anatomy、Usage、Machine Context、Machine
> API、Data Attributes、CSS Variables、Accessibility、Keyboard Interactions、源码、npm 包和 Logic Visualizer。对 AI
> 来说，`machine`、`connect`、parts getter、context props、data attributes、键盘表和示例形成了可检索、可组合、可改写的稳定语料。

#### `copy_modify_verify_loop`

> Zag.js 的常规循环是安装机器包和框架 adapter，使用 `useMachine` 启动服务，调用 `connect(service, normalizeProps)` 得到 api，再把
> `api.getRootProps()`、`api.getTriggerProps()`、`api.getContentProps()` 等 getter 展开到自定义 DOM 结构上。修改主要发生在应用 wrapper、样式和
> machine context 配置层；验证依赖 TypeScript、组件示例、Playwright/E2E、键盘交互和 a11y 行为。open-gpui 可把这个循环升级为 scaffold recipe 后运行
> contract、visual、interaction、a11y 和性能门禁。

### API 与组合

#### `api_ergonomics`

> API 形态是“机器 + adapter + getter”：组件包导出 `machine` 和 `connect`，框架包提供 `useMachine`、`normalizeProps`、`mergeProps` 等工具；调用者把
> machine context 作为配置传入，再用 api getter 给每个 part 绑定 id、role、aria、data-*、style 和事件处理器。优点是低层、可组合、可测试、跨框架一致；代价是比高层 JSX
> 组件更啰嗦，需要使用者理解 parts 和状态机配置。

#### `customization_model`

> 自定义分为四层：机器 context 控制行为，例如
> `open`、`defaultOpen`、`onOpenChange`、`positioning`、`modal`、`closeOnEscape`、`closeOnInteractOutside`；api getter 控制结构和
> parts；`mergeProps` 支持事件组合；样式完全由使用者通过 `data-part`、`data-state`、`data-disabled`、`data-highlighted`、`data-placement` 和 CSS
> variables 接管。深层行为可通过自定义机器或 fork 修改，但官方主路径是配置和 wrapper。

#### `component_anatomy_model`

> 非常明确。每个组件都有 anatomy 和 part names，DOM 上通过 `data-scope` 与 `data-part` 标识，例如 popover/menu 常见
> root、trigger、positioner、content、arrow、arrow-tip、item、separator、group、label 等 parts；复杂组件还会提供 hidden
> input、control、indicator、viewport 等 part。这个模型很适合 open-gpui 映射为 Element parts、Entity state、AccessKit 节点和可视化 gallery 元数据。

#### `state_ownership_model`

> Zag.js 支持 uncontrolled 与 controlled 两种状态所有权：常见机器提供
> `defaultOpen`/`open`/`onOpenChange`、`defaultValue`/`value`/`onValueChange`、`defaultHighlightedValue`/`onHighlightChange`
> 等模式；机器内部用 bindable context、computed、watch、refs、guards、actions、effects 表达状态和副作用。open-gpui 可借鉴为：应用可接管核心状态，primitive
> 内部负责事件转移、焦点、键盘、dismiss、定位和 a11y metadata，运行时 handle 只处理测量、focus 和平台副作用。

### Headless 与行为

#### `headless_boundary`

> 边界很清楚：机器层负责逻辑、状态、交互、可访问性和必要 DOM 查询；framework adapter 负责响应式绑定、props 归一化和事件合并；渲染结构由用户按 parts 组织；样式由用户控制。需要注意 Zag.js 仍依赖
> DOM scope、document/window、id 查询、aria 和 CSS data attributes。open-gpui 应保留机器/adapter 分层思想，但把 DOM scope 替换成
> GPUI/AccessKit/窗口与布局查询抽象。

#### `accessibility_model`

> Zag.js 以 WAI-ARIA authoring practices 为行为基线，机器处理 keyboard interactions、focus management、aria roles/attributes、aria-
> activedescendant、label/description 关系、hidden input、focus trap、focus return 等细节。组件文档会列出 Accessibility 与 Keyboard
> Interactions。open-gpui 不能照搬 ARIA 属性，但可把 role、label、value、action、relationship、focused/active
> descendant、disabled、expanded、checked、modal、hidden-from-screen-reader 等语义映射到 AccessKit contract。

#### `positioning_and_collision_model`

> Zag.js 在 popover、menu、tooltip、select 等 overlay 机器中提供 `positioning` 配置、`placement`、positioner/content/arrow parts、`data-
> placement` 和 arrow CSS variables，并处理 portal、modal、focus return、outside interaction、Esc 关闭、多 trigger
> 重新定位等行为。具体碰撞算法细节未在调研范围内完全展开，但 API 层已经把 overlay 几何、dismiss、focus、trigger identity 和视觉状态连成统一 contract；open-gpui 应把这些转为纯
> geometry + interaction policy。

#### `interaction_state_machines`

> 这是 Zag.js 最核心的能力。每个组件行为由有限状态机/状态图建模，机器包含 states、events、context、computed、watch、refs、guards、actions、effects，并提供 Logic
> Visualizer。官方 README 明确要求所有机器按 WAI-ARIA authoring practices 建模，并为每个机器写跨框架 E2E 测试。对 open-gpui 来说，Zag.js 是 renderer-
> neutral UI state contract 的高价值参考。

### 渲染与性能

#### `rendering_model`

> 默认落地是 Web DOM：机器不直接渲染 UI，而是通过 React/Vue/Solid/Svelte adapter 把状态映射为 DOM props、事件、style 和 data attributes。它不是 native
> retained UI、immediate mode、自绘或 GPU scene 框架。

#### `native_advantage`

> open-gpui 的优势应放在 Zag.js 不覆盖的 native 层：高性能文本和代码编辑、大树/表格/列表、低延迟输入、窗口级 overlay、多显示器/DPI、GPU 合成、原生拖拽、多窗口、AccessKit 集成和非 DOM
> 布局。Zag 的机器模型可以提升行为一致性，但 native 差异化不能靠复刻 DOM props，而应靠直接掌控场景图、布局、输入和辅助技术节点。

#### `web_ecosystem_advantage`

> Web 生态在 ARIA 经验、浏览器/屏幕阅读器兼容、CSS selector 和变量、npm 包传播、跨框架 adapter、文档/LLM 语料、Playwright/E2E 基础设施和真实组件案例上更强。open-gpui
> 不应追求兼容 `normalizeProps` 或 DOM data attributes 的表面形态，而应复用术语和 contract 思想，并在文档中给出 Web/Zag 到 GPUI primitive 的映射表。

### 主题与设计系统

#### `theme_token_model`

> Zag.js 本身不提供设计 token 或默认视觉主题；它只输出 parts、state、placement、disabled、highlighted、checked 等状态标记，以及部分 overlay/arrow CSS
> variables。主题、尺寸、颜色、动画、阴影和视觉变体由使用者或上层设计系统负责。open-gpui 应同样把 headless machine 和 theme token 解耦，让机器只输出状态与语义，theme recipe
> 再消费这些状态。

#### `design_token_pipeline`

> Zag.js 未体现 DTCG、Style Dictionary 或 Tailwind-like token transform 管线；它是 token/样式系统的下游行为层，而不是 token 编译系统。对 open-gpui 的启示是
> token pipeline 应作为独立 crate 或工具存在，向 wrapper 和 style recipe 提供主题数据；primitive machine 不应依赖具体 token 格式，只暴露稳定状态、part 和几何语义。

#### `style_customization_boundary`

> 样式边界几乎完全在用户侧。Zag.js 负责为 parts 和 states 自动附加 `data-*` 属性、必要 id、aria 和 style；用户使用任意 CSS 方案选择 `[data-part]`、`[data-
> state]`、`[data-disabled]`、`[data-highlighted]`、`[data-placement]` 等进行样式定制。open-gpui 可把这一层改造成 typed style state：核心
> primitive 输出状态枚举和 part id，theme recipe 与用户 Element 渲染负责视觉。

### 组件表面

#### `component_coverage`

> 覆盖很广，官方列表包含 Accordion、Angle Slider、Avatar、Carousel、Cascade
> Select、Checkbox、Clipboard、Collapsible、ColorPicker、Combobox、Date Input、Date Picker、Dialog、Drawer、Editable、File
> Upload、Floating Panel、Hover Card、Image Cropper、Listbox、Menu、Context Menu、Navigation Menu、Number
> Input、Pagination、Password Input、Pin Input、Popover、Presence、Progress、QR Code、Radio Group、Rating Group、Scroll
> Area、Segmented Control、Select、Signature Pad、Slider、Splitter、Steps、Switch、Tabs、Tags
> Input、Timer、Toast、Toggle、Tooltip、Tour、Tree View 等。

#### `must_have_for_open_gpui`

> 必须借鉴的是状态机优先的 primitive 设计、`machine`/`connect`/adapter 分层、parts anatomy、controlled/uncontrolled bindable 状态、keyboard/a11y
> contract、Logic Visualizer 思路、跨框架一致测试和 LLM 文档入口。open-gpui 的首批必须补齐能力应包括
> Dialog、Popover、Menu、Tooltip、Select/Combobox、Tabs、Checkbox、Radio、Switch、Slider、Toast、Scroll Area、Tree View，以及统一的
> focus、dismiss、overlay positioning 和 AccessKit 映射。

#### `do_not_chase`

> 当前阶段不应追 DOM prop getter 的逐字 API、React/Vue/Svelte adapter、CSS data attribute 语法、隐藏 input 表单兼容、npm 独立包拆分、全部长尾组件、Web ARIA
> 属性名和 Shadow DOM/iframe 细节。open-gpui 应追的是等价机器契约、可视化、测试和 native adapter，而不是把 Web 的 DOM 机制搬进 Rust。

### 治理

#### `versioning_and_breakage`

> Zag.js 通过 npm 包版本和 changelog 管理破坏性变化；FAQ 提到由于 `@zag-js/*` 独立版本管理，批量升级可能不便，可用 package manager 的 scoped upgrade
> 一次更新。组件机器独立发包有利于增量采用，但也要求共享 adapter、机器包和文档版本保持一致。open-gpui 若采用类似拆分，应明确核心 contract 的 SemVer、实验机器前缀、migration guide 和跨
> crate 兼容矩阵。

#### `maintenance_cost`

> 维护成本高但收益也高。每个 primitive 都要维护状态机、context、computed、watch、guards、actions、effects、DOM/platform
> scope、a11y、键盘、焦点、定位、docs、examples、LLM 文档、visualizer 和跨框架 E2E。open-gpui 如果全量追 Zag 覆盖面会过重，更现实的是先做少数核心机器的深 contract
> 和测试工具链，再复制模式扩展。

#### `risks`

> 主要风险是把 Web DOM 机制误当成通用抽象：id 查询、document/window、ARIA 属性、data attributes、CSS variables 和 framework spread props 不能原样成为
> native Rust API。第二个风险是状态机过度细化导致 API 学习和调试成本上升。第三个风险是第三方 recipe 若没有机器可读 contract，会出现组件碎片化和 AI
> 生成不可验证。第四个风险是行为层很强但渲染/布局/性能层缺位，导致 open-gpui 的 native 优势没有体现。

#### `open_gpui_relevance`

> 建议 adopt 核心思想、trial 少量机器、reject Web 运行时形态。Zag.js 是 open-gpui 设计 renderer-neutral UI primitive contract 的关键参考：先定义 Rust
> typed state machine、part anatomy、event table、AccessKit mapping、focus policy、overlay geometry 和 diagnostics，再由 GPUI
> Element adapter 消费。直接设计含义是 open-gpui 通用 UI 框架应优先成为“native headless state machine primitives + adapter + recipe/gallery +
> contract tests”，而不是先做完整视觉组件库。

### 不确定字段（已跳过）

- `diagnostics_and_failure_quality`
- `docs_gallery_model`
- `machine_readable_contracts`
- `performance_model`
- `registry_viability`
- `rust_distribution_fit`
- `testing_strategy`
- `third_party_ecosystem_path`

## <a id="ark-ui"></a>7. Ark UI

- 结果文件：`Ark_UI.json`
- 调研类别：`headless_cross_framework_components`
- 纳入原因：
  > 基于 Zag 的跨 React/Vue/Solid/Svelte headless components；适合研究 multi-framework adapter 和 anatomy/part API。
- 参考来源：
  - https://ark-ui.com/

### 定位

#### `positioning`

> Ark UI 的定位是跨 React、Solid、Vue、Svelte 的 headless 组件库和设计系统基础设施。它把复杂交互、可访问性、DOM 绑定、组件 anatomy 与框架 adapter 组织在一起，底层行为来自
> Zag.js 有限状态机，而视觉样式完全留给应用或设计系统。

#### `target_users`

> 主要服务需要跨多个前端框架维护同一套设计系统的团队、Web 应用开发者、组件库维护者、Chakra/Park/Tailwind/Panda CSS 生态使用者，以及希望 AI 能按组件文档生成代码的前端工程师。

#### `primary_value_proposition`

> 核心价值是用一套状态机驱动的组件行为同时落到多个 JS 框架，避免每个框架重复实现 menu、select、dialog、combobox、popover 等复杂交互。它与 open-gpui 的匹配点很强：可参考其 renderer-
> neutral 行为内核、跨 adapter API parity、anatomy/part 命名和状态机测试思路；不匹配的是 DOM、ARIA、CSS 和 npm 运行时形态。

### 分发与生态

#### `distribution_model`

> Ark UI 采用 package dependency 分发，按框架发布 `@ark-ui/react`、`@ark-ui/solid`、`@ark-ui/vue`、`@ark-ui/svelte` 等 npm
> 包，并通过子路径导入单个组件、`/anatomy`、`/factory`、providers 和 utilities。它不是 shadcn 式 copy-to-own registry，也不是组件源码 CLI add
> 模式；示例、文档、MCP、llms.txt 和 Park UI/Tark UI 等样式层构成外围生态。

#### `source_ownership`

> 使用者默认不拥有本地组件源码，而是依赖 MIT 开源包；可以阅读源码、fork 或 patch，但常规使用路径是安装包并在本地设计系统中包一层样式和 API。升级成本主要来自 Ark 包版本、Zag.js 版本、框架 adapter
> 差异和本地 wrapper 的适配；相比 copy-to-own，行为修复集中，但深度改状态机需要理解 Zag 或维护 fork。

### AI 时代设计

#### `ai_friendliness`

> 较高。Ark 官方文档有 Markdown 入口、ChatGPT/Claude 打开入口、MCP server、llms.txt、llms-full.txt 以及按 React/Solid/Vue/Svelte 分开的 LLM
> 文档；组件页面包含 anatomy、examples、controlled/root provider、API reference、data attributes、CSS variables 和 keyboard support。对 AI
> 来说，它比普通组件库更容易检索、组合和生成，但生成后的行为验证仍主要依赖项目自身测试。

### API 与组合

#### `api_ergonomics`

> API 以 namespace + anatomy parts 为核心，例如
> `Dialog.Root`、`Dialog.Trigger`、`Dialog.Backdrop`、`Dialog.Positioner`、`Dialog.Content`、`Dialog.Title`、`Dialog.Description`、`Dialog.CloseTrigger`。复杂组件支持
> `RootProvider` 和 `useComponent` hook 控制外部状态，所有渲染 DOM 的部件通常支持 `asChild`，并通过 `ids` 组合多个组件的可访问性关系。调用体验一致，适合设计系统封装和跨框架迁移。

#### `customization_model`

> 定制分为结构、样式和行为三层：结构通过 anatomy parts、`asChild`、factory、自定义 ids 和 RootProvider 组合；样式通过 data-scope/data-part/data-
> state、class/className、CSS variables、Panda slot recipe、Tailwind 或任意 CSS 方案；行为通过 controlled props、default props、onChange
> 回调、positioning、modal、closeOnEscape、closeOnInteractOutside、persistentElements 等参数调整。深层行为算法仍在 Zag/Ark 内部。

#### `component_anatomy_model`

> 非常明确。每个复杂组件都公开 Root、Trigger、Positioner、Content、Item、Indicator、Arrow、ArrowTip、CloseTrigger、Context、RootProvider 等
> parts，并在文档中列出 anatomy 示例和 data-part。Ark 还提供 `@ark-ui/<framework>/anatomy` entrypoint，方便 Panda CSS 的 slot recipe 使用同一组
> parts。这个模型非常适合 open-gpui 映射为行为 Entity、Element parts、theme slots、AccessKit 节点和 gallery 元数据。

#### `state_ownership_model`

> Ark 支持多层状态所有权：组件内部可用 defaultOpen/defaultValue 等非受控方式自治；应用可通过 open/value/onChange 等 props 受控；需要跨树或外部触发时可用 `useComponent`
> 加 `RootProvider`；局部子组件可通过 `Component.Context` 或 `use*Context` 读取状态和方法。对 open-gpui 的启发是把组件内部瞬态状态、应用业务状态、可提升状态、运行时 handle
> 和可序列化配置明确拆开。

### Headless 与行为

#### `headless_boundary`

> 边界整体清楚：Zag.js 负责有限状态机、事件、可访问性属性、焦点和交互逻辑；Ark adapter 把机器输出绑定到 React/Solid/Vue/Svelte 的组件和 DOM props；样式和视觉 theme
> 留给用户；定位、dismissable layer、presence、portal 等以组件 prop 和 CSS variables 暴露。对 open-gpui 来说，应保留“行为内核与渲染 adapter 分离”的思想，但把
> DOM/ARIA/CSS 替换为 GPUI Element、窗口 overlay、AccessKit 和 Rust 类型。

#### `accessibility_model`

> Ark 以 WAI-ARIA 模式、ARIA 属性、role、键盘导航、焦点管理、screen reader announcement、RTL 支持和 ids 组合为主要可访问性模型；组件页面通常列出 keyboard
> support，Popover 等组件还支持 modal、focus trap、outside interaction、escape 关闭和 focus return。open-gpui 不能照搬 ARIA
> 字符串，但应借鉴其显式键盘表、label/description/trigger/content 关系和焦点策略，并映射到 AccessKit 的 role、name、value、action、relationship 和 focus
> order。

#### `positioning_and_collision_model`

> Ark 的 overlay 组件通过 `Positioner`、`Anchor`、`Arrow`、`ArrowTip`、`positioning` prop、available size CSS
> variables、`--x`、`--y`、`--transform-origin`、`--layer-index`、`--z-index` 等暴露定位结果；Popover 支持 placement、sameWidth、嵌套层、多个
> trigger、modal、portal、dismiss 和 reposition 方法。open-gpui 应把这些概念转成纯 geometry contract：anchor rect、viewport、preferred
> placement、collision policy、arrow data、available size、layer index、focus return 和 dismiss policy。

#### `interaction_state_machines`

> 这是 Ark 最值得参考的核心。Ark 基于 Zag.js，每个组件行为由有限状态机驱动，官方 README 明确强调 type-safe state transitions、test/debug easier、fewer edge
> cases 和 visualizable component logic。对 open-gpui 来说，menu、select、dialog、popover、combobox、tabs、tree、listbox、toast 等
> primitive 应有显式状态、事件、转移、guards、effects 和测试用例，而不是把交互散落在渲染代码中。

### 渲染与性能

#### `rendering_model`

> Web DOM 渲染模型：Zag 状态机输出 DOM props，Ark 的框架 adapter 渲染 React/Solid/Vue/Svelte 组件，浏览器负责布局、绘制、事件、滚动和无障碍树。它不是 native retained
> UI、immediate mode、自绘或 GPU scene 框架。

#### `native_advantage`

> open-gpui 可以在 Ark 不覆盖的区域形成明显 native 优势：大文本和代码编辑、长列表/树/表格、低延迟输入、GPU 合成、多窗口多显示器坐标、原生菜单/拖拽、精确滚动所有权、窗口级 overlay 调度和
> AccessKit 集成。Ark 的价值在行为正确性和跨 adapter 架构，不在底层渲染性能。

#### `web_ecosystem_advantage`

> Web 生态在 Ark 所处领域有天然优势：ARIA 与屏幕阅读器链路成熟，CSS/Panda/Tailwind/Storybook/Chromatic/npm 生态强，React/Vue/Svelte 用户基数大，已有大量设计系统和样式
> recipes。open-gpui 不应早期追逐完整 Web 组件生态，而应保持术语和 anatomy 可迁移，优先做好原生桌面高密度 UI 的行为和性能优势。

### 主题与设计系统

#### `theme_token_model`

> Ark UI 本身不提供完整 theme token 系统，而是通过 headless parts、data attributes、CSS variables 和 anatomy entrypoint 供外部样式系统消费。文档推荐
> Panda CSS 的 `defineSlotRecipe` 和 anatomy keys，也展示 Tailwind、CSS Modules、vanilla CSS 等方式；z-index 推荐使用共享 overlay token 加
> `--layer-index`。open-gpui 应把 primitive 行为层和 theme token 层明确分离。

#### `style_customization_boundary`

> 样式边界很清楚：Ark/Zag 负责行为、状态、必要 DOM 属性和功能性定位样式；用户或设计系统负责 presentation style、class、variant、token、动画、布局和外观。data-state、data-
> scope、data-part、data-placement、CSS variables 是行为层与样式层之间的契约。open-gpui 可把这层改成 typed style state + theme recipe + user
> Element override，而不是复制 CSS 选择器。

### 组件表面

#### `component_coverage`

> 覆盖面很广，包含基础、表单、overlay、导航、数据展示和 utilities：Accordion、Angle Slider、Avatar、Carousel、Checkbox、Clipboard、Collapsible、Color
> Picker、Combobox、Date Input、Date Picker、Dialog、Drawer、Editable、Field、Fieldset、File Upload、Floating Panel、Hover
> Card、Listbox、Marquee、Menu、Number Input、Pagination、Password Input、Pin Input、Popover、Progress、QR Code、Radio Group、Rating
> Group、Scroll Area、Segment Group、Select、Signature Pad、Slider、Splitter、Steps、Switch、Tabs、Tags
> Input、Timer、Toast、Toggle、Tooltip、Tour、Tree View，以及 Client Only、Environment、Focus Trap、Locale、Presence、Portal 等工具。

#### `must_have_for_open_gpui`

> 必须重点借鉴的是 Zag 式有限状态机、跨 adapter 分层、Root/Trigger/Content/Item/Positioner anatomy、受控/非受控/RootProvider 状态模型、data-to-style
> contract、overlay layer 与定位变量、AI 文档入口和 anatomy entrypoint。open-gpui 通用 UI 框架至少应先用这种方式补齐
> Dialog、Popover、Menu、Tooltip、Select、Combobox、Tabs、Checkbox、Radio、Switch、Slider、Toast、Scroll Area、Tree/Listbox 这些高复用
> primitive。

#### `do_not_chase`

> 当前阶段不应追 Ark 的 React hook API、DOM prop getter、ARIA 属性字符串、CSS selector 细节、npm 多框架发布矩阵、Web 表单隐式行为、所有预览组件、Panda/Tailwind
> 绑定和完整 Web 组件数量。open-gpui 应追等价的行为语义、状态机 contract、AccessKit 映射和原生渲染优势。

### 治理

#### `versioning_and_breakage`

> Ark 通过 npm 包版本、Changesets、changelog、框架包同步版本和 Zag 依赖版本治理破坏性变更；当前文档显示 v5.37.2，仓库 release 也以各框架包发布。多框架 API parity 增加了
> breaking change 成本，因为同一组件 contract 要同时落到 React/Solid/Vue/Svelte。open-gpui 应学习稳定核心 contract、实验组件标记、schema
> version、migration guide 和 adapter 兼容矩阵。

#### `maintenance_cost`

> 维护成本高。Ark 要同时维护 Zag 状态机、四个框架 adapter、DOM/ARIA/focus/keyboard/positioning 行为、类型、文档、示例、MCP、LLM 资料、Storybook 和发布流程。open-
> gpui 若采用类似架构，初期不应追完整组件数量，而应先把状态机内核、adapter contract、可访问性映射、overlay kernel 和测试工具做深，再扩展组件覆盖。

#### `risks`

> 主要风险是把 Web DOM/ARIA/CSS 细节机械搬到 native Rust，导致抽象错位；跨 adapter API parity 可能牺牲 GPUI 原生优势；状态机过度泛化会提高学习和调试成本；组件 anatomy 过细会增加
> API 面；没有机器可读 contract 时 AI 生成仍不可验证；若同时推进组件数量、主题、registry、docs 和 MCP，容易让核心团队维护负担过重。

#### `open_gpui_relevance`

> 建议为 trial 偏 adopt：不要采用 Ark 的 Web 运行时，但应采用它的架构思想。直接设计含义是 open-gpui 应优先设计 renderer-neutral 状态机/行为 contract、GPUI Element
> adapter、AccessKit adapter、overlay geometry service、anatomy/part API、theme slot recipe 和 AI 可读 docs。第一批试点应选
> Dialog、Popover、Menu、Select 或 Combobox，验证一套行为内核能否稳定服务多个视觉 wrapper 和示例。

### 不确定字段（已跳过）

- `copy_modify_verify_loop`
- `design_token_pipeline`
- `diagnostics_and_failure_quality`
- `docs_gallery_model`
- `machine_readable_contracts`
- `performance_model`
- `registry_viability`
- `rust_distribution_fit`
- `testing_strategy`
- `third_party_ecosystem_path`

## <a id="base-ui"></a>8. Base UI

- 结果文件：`Base_UI.json`
- 调研类别：`unstyled_accessible_components`
- 纳入原因：
  较新的无样式 accessible React components，由 Floating UI/Radix/MUI 相关实践者推动；应作为 Radix 后继/竞争参考。
- 参考来源：
  - https://base-ui.com/

### 定位

#### `positioning`

> Base UI 的生态定位是面向 React/Web DOM 的无样式、可访问组件库与 headless primitive 集合。它不是视觉设计系统、token pipeline
> 或完整应用框架，而是把复杂组件的行为、可访问性、焦点管理、键盘交互、弹层定位和可组合 anatomy 封装成稳定 React 组件；相对 Radix，它更像由 Radix、Floating UI、Material UI
> 经验重新整合后的后继/竞争参考。

#### `target_users`

> 主要服务 React 应用开发者、设计系统作者、unstyled 组件库维护者、需要高质量可访问交互的产品团队，以及希望基于底层 primitives 构建自有视觉体系的团队。对 open-gpui 来说，真正的参考对象是原生 UI
> 框架维护者和桌面产品团队，而不是直接消费 React 组件的用户。

#### `primary_value_proposition`

> 核心价值是用一个稳定、tree-shakable 的包提供完整可访问组件面，同时不绑定 CSS 或视觉风格，让使用者完全拥有样式层。它与 open-gpui 的匹配点在于组件 anatomy、受控/非受控状态、render
> 替换、事件原因、弹层 Positioner、焦点/键盘/a11y contract 和面向 LLM 的文档组织；不匹配点是 React DOM、CSS 变量、ARIA 属性和 npm 生态本身。

### 分发与生态

#### `distribution_model`

> Base UI 主要以 npm package dependency 分发：安装单一 `@base-ui/react` 包，通过子路径如 `@base-ui/react/popover` 按需导入，包声明 sideEffects false
> 并强调 tree-shakable。官方还提供每个 master commit 和 PR 的 pkg.pr.new canary 安装方式；预样式组件不在核心包内，而是通过 shadcn/ui first-class
> integration、styled libraries 和社区封装进入生态。它本身不是 copy-to-own registry 或 CLI add 模式，但可作为 shadcn copy-to-own 组件的底层行为依赖。

#### `source_ownership`

> 默认情况下使用者不拥有组件源码，而是依赖 MIT 开源 npm 包；可以查看源码、fork、patch-package 或包一层自有组件，但常规升级路径是跟随 `@base-ui/react` 版本。通过 shadcn/ui
> 集成时，用户可能拥有 wrapper 和样式源码，但底层行为仍由 Base UI 包提供。升级成本主要来自 API/行为变更、React 版本兼容、事件语义变化和自定义 wrapper 的适配；相比源码
> registry，行为修复接收更集中，但深度改内部算法需要 fork。

#### `third_party_ecosystem_path`

> Base UI 的第三方路径很清楚：核心包保持无样式，shadcn/ui 提供 Tailwind wrapper 和 CLI/create 入口，社区 styled libraries 例如 Kumo、coss
> ui、Fragments、Prototyper UI、Lumi UI、Olyx UI、ReUI、Fig UI、Selia 等在上层构建视觉系统，另有 Solid/Vue 非官方移植。open-gpui 可借鉴为“核心 primitive
> 稳定、第三方在 wrapper/theme/recipe/gallery 层创新”，并要求第三方提供版本兼容范围、示例、截图、a11y contract 和交互测试。

### AI 时代设计

#### `ai_friendliness`

> 很高。官方文档明确提供 `llms.txt` 和每个页面的 Markdown 版本，页面结构包含 quick start、handbook、组件 anatomy、API reference、TypeScript
> Props/State、data attributes、CSS variables、示例代码和 release notes。对 AI 来说，`Root/Trigger/Portal/Positioner/Popup/Item`
> 这类稳定词汇、组件状态类型、事件 reason、render prop 和 markdown 文档都便于检索、组合、迁移和验证。

#### `copy_modify_verify_loop`

> Base UI 的默认循环是安装包、按 anatomy 组装 parts、用 Tailwind/CSS Modules/plain CSS/CSS-in-JS 样式化，并通过
> TypeScript、浏览器行为、键盘、焦点和屏幕阅读器路径验证。若使用 shadcn/ui，则 wrapper 和样式可进入 copy-to-own 循环，但底层 Base UI 行为仍以依赖包升级。open-gpui 应把该循环升级为
> recipe 生成后可本地修改，并用 contract tests、visual snapshots、interaction tests、a11y metadata tests 和性能门禁验证。

### API 与组合

#### `api_ergonomics`

> API 形态是声明式 parts 组合与命名空间类型：典型组件由 `Root` 持有状态和上下文，`Trigger` 触发，`Portal` 脱离渲染层级，`Positioner`
> 负责定位，`Popup/Item/Indicator/Arrow/Close/Viewport` 等暴露细粒度结构。组件支持 `className`/`style` 字符串或基于 state 的函数，支持 `render` prop
> 替换底层元素并组合自有组件，支持 controlled/uncontrolled props 和 `onOpenChange`/`onValueChange` 等带 reason 的事件。复杂场景还提供
> `createHandle`、detached trigger、`actionsRef`、`keepMounted` 等逃生口。

#### `customization_model`

> 样式完全由用户控制：核心包不打包 CSS，兼容 Tailwind、CSS Modules、plain CSS、CSS-in-JS。状态通过 data attributes、CSS
> variables、`className(state)`、`style(state)` 和 TypeScript `State` 暴露；结构通过 `render` prop 替换默认元素；行为通过 controlled
> props、change event details 的 `cancel()`/`allowPropagation()`、`preventBaseUIHandler()`、focus
> props、delay、modal、keepMounted、actionsRef 等调整。底层算法仍在库内，深度改动需要 fork 或等待上游 API。

#### `component_anatomy_model`

> 组件 anatomy 非常明确。Popover 这类弹层组件拆为
> `Root`、`Trigger`、`Portal`、`Backdrop`、`Positioner`、`Popup`、`Arrow`、`Viewport`、`Title`、`Description`、`Close`；Select/Menu/Combobox/Tabs/Toast
> 等也有 Item、List、Value、Indicator、Provider、Viewport 等 parts。该模型适合 open-gpui 映射为行为实体、渲染 Element、overlay host、focus
> scope、slot/part 和 theme recipe，而不是做一个不可拆的单体 widget。

#### `state_ownership_model`

> 默认非受控，组件内部管理 open/value/checked/pressed/highlight/focus 等状态；需要接入应用状态时，可通过 `open`/`value`/`checked` 等 props 与
> `onOpenChange`/`onValueChange`/`onPressedChange` 提升为受控模式。事件详情包含 reason、原生 event、cancel、allowPropagation
> 等信息，允许外部细粒度拦截内部状态变化。部分组件暴露 handle 或 actionsRef 处理 detached triggers、手动 open/close/unmount 等命令式路径。open-gpui
> 可借鉴“组件自治优先，应用可接管关键状态，运行时 handle 只处理测量/焦点/命令”的边界。

### Headless 与行为

#### `headless_boundary`

> 边界整体清楚：Base UI 负责行为逻辑、可访问性语义、键盘、焦点、表单集成、弹层定位、dismiss、portal 和状态到样式 hook 的暴露；用户负责视觉样式、设计 token、布局细节和高层 wrapper API。需要注意其
> headless 层仍强绑定 React、DOM、ARIA、CSS variables、portal 和浏览器事件。open-gpui 应抽象为 renderer-neutral 行为 contract，再由 GPUI
> Element、AccessKit、window/overlay host 和 theme adapter 落地。

#### `accessibility_model`

> Base UI 把 accessibility 作为主要目标，文档明确说明组件处理 ARIA 属性、role 属性、pointer interactions、keyboard navigation 和 focus
> management，并遵循 WAI-ARIA Authoring Practices。许多组件支持方向键、字母键、Home、End、Enter、Esc；部分组件提供 `initialFocus`、`finalFocus`
> 等焦点配置；Field/Form/Input/Fieldset 自动关联表单控制。开发者仍需负责可见焦点样式、颜色对比和业务层 accessible name。open-gpui 需要把这些语义转译为 AccessKit 的
> role、label、value、action、relationship、focus order 和 keyboard contract。

#### `positioning_and_collision_model`

> Base UI 在弹层族组件中显式提供 `Portal`、`Positioner`、`Arrow`、`Viewport`、side/align/offset、transform origin、available size、anchor
> size、side/align data attributes 和 CSS variables，并依赖 `@floating-ui/react-dom`。release notes 多次修复
> Dialog/Menu/Popover/PreviewCard/Tooltip 的 positioning、viewport、focus return、inline anchoring 和 detached trigger 问题。open-
> gpui 应借鉴 Positioner 作为独立 part 的设计，但把 CSS 变量和 DOM measurement 替换为原生 geometry contract、window/viewport boundary、collision
> policy、arrow data 和 diagnostics。

#### `interaction_state_machines`

> Base UI 没有把有限状态机作为显式公开 API 暴露，但通过 `State` 类型、data attributes、change event reason、keyboard/focus/dismiss
> 行为、controlled/uncontrolled props 和 actionsRef 形成了可测试的等价 contract。对 open-gpui 来说，应进一步显式化为状态图与事件表，尤其是
> Menu、Select、Combobox、Dialog、Popover、Toast、Tabs、Slider、Toolbar 等组件，避免行为散落在渲染代码里。

### 渲染与性能

#### `rendering_model`

> Base UI 的落地模型是 React DOM：组件渲染 HTML 元素，使用 React context、refs、portal、DOM events、CSS transitions/animations、CSS variables 和
> data attributes。它不是 native retained UI、immediate mode、自绘或 GPU scene 框架。

#### `performance_model`

> 性能策略集中在 Web 组件层：单包 tree-shakable、sideEffects false、按子路径导入；release notes 提到 closed popup mount/unmount 性能优化、Combobox
> 避免每次输入重渲染所有 item、Drawer swipe gesture 使用更接近原生的驱动方式、detached trigger 性能改善。仓库脚本包含 bundle-size、浏览器测试、e2e/regression 和性能
> benchmark。它不提供大表格、大树、大文本、canvas 或 native incremental render 策略，因此 open-gpui 的性能差异化不应来自复刻 Base UI，而应来自原生渲染、布局和数据结构。

#### `native_advantage`

> open-gpui 应在 Base UI/Web DOM 不擅长的场景形成优势：大文本/代码编辑、大表格/树/列表、低延迟输入、GPU 合成、窗口级 overlay、多窗口/多显示器/DPI、命令面板、复杂 docking、原生拖拽和跨平台
> AccessKit 集成。Base UI 可作为行为正确性标杆，但 native GPUI 应把这些行为放到更低延迟、更可诊断、更适合桌面产品的渲染和事件模型中。

#### `web_ecosystem_advantage`

> Web 生态天然更强的是 WAI-ARIA/APG、浏览器与屏幕阅读器兼容经验、CSS 选择器和动画、npm 分发、React 设计系统、shadcn copy-to-own 流程、第三方 styled
> libraries、社区示例和真实线上验证。open-gpui 不应在早期硬追这些生态规模，而应保持术语和行为对齐，提供迁移心智，并优先做 native 更强的 primitive、测试和 diagnostics。

### 主题与设计系统

#### `theme_token_model`

> Base UI 核心没有视觉 theme token 模型，不提供颜色、尺寸、圆角、阴影、密度或 semantic token。它只提供状态 data attributes、state 函数、CSS variables 和组件
> parts，让 Tailwind、CSS Modules、CSS-in-JS、shadcn wrapper 或第三方 styled library 决定主题。open-gpui 应把 primitive 行为层与 theme token
> 层分离：核心只输出状态、几何、a11y 和交互信息，theme recipe 再消费这些信息。

#### `design_token_pipeline`

> Base UI 不提供 DTCG、Style Dictionary 或 Tailwind-like transform pipeline；Tailwind 示例只是消费方式，shadcn/styled libraries
> 才是上层样式封装。对 open-gpui 的启示是 token pipeline 应是独立系统，可把 DTCG/Style Dictionary/自定义 Rust schema 转为 GPUI theme tokens、component
> recipe 和 gallery preview，而不要塞进 headless primitive crate。

#### `style_customization_boundary`

> 样式边界明确在用户侧和 wrapper 层：framework 负责行为、状态、语义和必要几何；component props 主要承载行为与结构；`className`/`style`/data attributes/CSS
> variables 连接状态与样式；设计系统 wrapper 或 app adapter 负责 token、variant、尺寸、动画和最终视觉。open-gpui 可对应为 core primitive、theme
> recipe、component wrapper、app adapter 四层，避免核心 widget 同时承担行为和视觉政策。

### 组件表面

#### `component_coverage`

> 覆盖面已经接近完整基础组件库：Accordion、Alert Dialog、Autocomplete、Avatar、Button、Checkbox、Checkbox Group、Collapsible、Combobox、Context
> Menu、Dialog、Drawer、Field、Fieldset、Form、Input、Menu、Menubar、Meter、Navigation Menu、Number Field、OTP Field、Popover、Preview
> Card、Progress、Radio、Radio Group、Scroll Area、Select、Separator、Slider、Switch、Tabs、Toast、Toggle、Toggle
> Group、Toolbar、Tooltip，以及 CSP Provider、Direction Provider、mergeProps、useRender 等 utilities。缺口主要是数据表格、树、虚拟列表、应用 shell、rich
> editor 这类更重的产品级组件。

#### `must_have_for_open_gpui`

> 必须借鉴的是 anatomy/parts、无样式边界、受控/非受控状态、event reason/cancel、render/slot 替换、Positioner、focus management、keyboard
> contract、state-to-style hook、Markdown/LLM docs 和 release/test 纪律。open-gpui 通用 UI 框架至少应优先补齐
> Dialog、Popover、Tooltip、Menu、Select/Combobox、Tabs、Checkbox、Radio、Switch、Slider、Toast、Scroll Area、Toolbar、Field/Form 这些行为
> primitive，再考虑视觉组件和高阶 recipes。

#### `do_not_chase`

> 当前阶段不应追逐 React DOM API、`render` prop 的逐字形态、CSS data attribute 命名、CSS variable 体系、HTML form/ARIA 细节、npm/shadcn 的完整生态复制、所有
> Base UI 组件覆盖面、Web 动画库适配或浏览器特殊兼容逻辑。open-gpui 应追等价行为语义、AccessKit 映射、原生 overlay kernel、测试 contract 和 native 性能优势。

### 治理

#### `versioning_and_breakage`

> Base UI 通过 npm SemVer、release notes、GitHub releases 和 canary releases 管理版本。官方文档显示 v1.0.0 于 2025-12-11 稳定发布，当前调研时最新
> timeline 为 v1.6.0（2026-06-17/页面标题为 Jun 18, 2026），并记录 breaking change、preview 到 stable、包名从 `@base-ui-components/react` 改为
> `@base-ui/react` 等迁移信息。open-gpui 应学习稳定 API、preview/unstable API、canary、migration guide 和 package validation 的分层治理。

#### `maintenance_cost`

> 维护成本很高：每个 primitive 都要长期维护行为、焦点、键盘、ARIA/a11y、弹层定位、表单集成、React 版本、浏览器差异、移动端输入、动画生命周期、文档示例、类型和跨浏览器测试。对 open-gpui
> 来说，直接追完整覆盖面风险很大；更合理是先建立少数高复用 primitive 的深 contract、测试基座和 diagnostics，再逐步扩展组件面。

#### `risks`

> 主要风险是把 Base UI 的 Web 形态机械搬到 native Rust，导致 React/DOM/ARIA/CSS 抽象错位；第二个风险是组件 parts 过细但缺少 Rust 端 schema 和测试，会提高学习成本并让 AI
> 生成不可验证；第三个风险是主题、registry、gallery、组件覆盖面同时推进导致维护爆炸；第四个风险是只复刻无样式 API 而没有原生性能、AccessKit 和 diagnostics 优势，最终变成 Web 组件库的低配移植。

#### `open_gpui_relevance`

> 建议为 trial：把 Base UI 作为 Radix 后继/竞争参考，试点其 anatomy、event details、Positioner、state-to-style、Markdown/LLM docs 和
> release/test 纪律；React DOM 实现本身只作 reference-only。直接设计含义是 open-gpui 应先定义 renderer-neutral headless primitive
> contract，再分别落到 GPUI Element、AccessKit、overlay geometry kernel、theme recipe、docs/gallery 和测试工具链；视觉组件和第三方 registry 应建立在这些
> contract 之上，而不是先做一批不可验证的 styled widgets。

### 不确定字段（已跳过）

- `diagnostics_and_failure_quality`
- `docs_gallery_model`
- `machine_readable_contracts`
- `registry_viability`
- `rust_distribution_fit`
- `testing_strategy`

## <a id="fret"></a>9. fret

- 结果文件：`fret.json`
- 调研类别：`local_reference_frontend_inspired_native_ui`
- 纳入原因：
  本地参考项目，包含 shadcn parity、组件 gallery、诊断脚本和前端组件库心智；deep 阶段应读 repo-ref/fret。

### 定位

#### `positioning`

> Fret 是本地参考中的 GPU-first Rust 桌面/wasm UI framework，同时内置组件生态、headless primitives、docs/gallery、诊断脚本、scaffold/CLI 与
> token/theme 基础设施；对本研究更像“原生 UI 框架 + shadcn/Radix 心智的生态样板”，不是单纯组件库。

#### `target_users`

桌面产品团队、编辑器级应用开发者、Rust 原生 UI 框架作者、设计系统/组件库维护者、需要可诊断 UI 自动化的 AI agent 与库维护者。

#### `primary_value_proposition`

> 把 Web 成熟的 shadcn/Radix/APG/Floating/cmdk 心智转译为非 DOM 的 Rust 原生 UI：核心 runtime 保持机制层，组件/策略在 ecosystem 层收敛，并用
> gallery、goldens、diagnostics 和 ADR 锁定行为。与 open-gpui 目标高度匹配，尤其在 headless 边界、组件 parity、AI 可验证性和原生性能差异化上有直接参考价值。

### 分发与生态

#### `distribution_model`

> 主要是 Cargo package dependency + workspace crates + feature flags + fretboard scaffold/template。应用通过 `fret`、`fret-ui-
> shadcn`、`fret-ui-kit` 等 crate 依赖消费；`fretboard new hello/simple-todo/todo/workbench-lite` 生成 starter 源码；assets/icons 通过生成
> manifest、AssetBundleId、IconRegistry 和 installer 组合。没有看到 shadcn 式 `registry add` 复制组件源码流程；`registry` 更多用于 diagnostics
> script catalog。

#### `source_ownership`

> 组件源码通常由 crate 维护者拥有，应用通过 Cargo/SemVer/feature flags 升级；scaffold 生成的 app 代码由用户拥有。优点是升级集中、行为 gate 可复用；代价是用户 patch/fork
> 组件要走 Cargo fork/patch 或上游贡献，不像 shadcn copy-to-own 那样每个组件天然进入 app 源码。

#### `rust_distribution_fit`

> 适配度高：workspace + crates.io 版本依赖、Cargo features、SemVer、rust-toolchain、profile 调优、模板 Cargo.toml 都是 Rust 原生分发方式。`fretboard`
> 充当 cargo-generate/xtask 风格的 CLI，能生成模板、运行 native/web、处理 assets/config/diag。第三方包可用 install 函数、feature-gated app-
> integration、AssetBundleId::package 与 icon registry 接入。

### AI 时代设计

#### `ai_friendliness`

> 很强：ADR 驱动、repo-ref 指向、gallery 作为可运行事实源、snippet-backed docs、source-policy tests、diagnostics bundle/schema2/test-id
> index/AI packet/reason_code 让 AI 能检索、定位、复现、修改、验证。README 也明确大量代码/文档由 AI 生成，因此项目在制度上依赖可追踪 contract 与 gate 来约束 AI 输出。

#### `machine_readable_contracts`

> 已具备多类机器可读契约：theme JSON、shadcn theme assets、diagnostics bundle.schema2.json、test_ids.index.json、script.result.json、diag
> script JSON、suite manifest、Cargo feature manifests、clap CLI contracts、semantics snapshot schema、source-policy
> tests。缺口是还没看到统一的 typed component registry schema 把 docs/gallery/scaffold/tests 全部从一个 manifest 派生。

#### `copy_modify_verify_loop`

> Fret 的循环不是单组件 copy-to-own，而是 scaffold/示例源码 + crate 依赖 + 本地修改 + nextest/source-policy/diagnostics/gallery gates。UI
> Gallery 要求 Preview 与 Code 由 snippet-backed source 保持一致；diag suite 可用语义选择器复现交互；web-vs-Fret/parity tests 和 component docs
> surface tests 验证修改。对 AI 来说，这是“生成/修改源码后跑合约与诊断脚本”而非“复制文件后手工比对”。

### API 与组合

#### `api_ergonomics`

> 应用层 API 是 `FretApp -> View -> AppUi -> Ui`，配合 `LocalState`、typed actions、`.action/.action_payload`、`cx.data()`
> selector/query、`ui::children!` 与 `.ui()` fluent builder。组件层使用 `facade as shadcn, prelude::*`，typed
> `IntoUiElement<H>`、builder pattern、variant/size props、parts API 和 explicit raw/advanced escape hatches。整体倾向强类型、可组合、但比
> JSX/CSS 更显式。

#### `customization_model`

> 样式通过 Theme/ThemeSnapshot token、ColorRef/MetricRef/Space/Radius、ChromeRefinement/LayoutRefinement 和 `.ui()` patch 定制；结构通过
> typed children、parts、raw namespace 和 recipe-level builder 定制；行为通过 typed actions、callbacks、controlled/uncontrolled
> model、headless primitives/action hooks 定制。escape hatch 明确在 `raw::*`、`advanced::*`、lower-level ElementContext/AnyElement。

#### `component_anatomy_model`

> 复杂组件大量采用 Radix/shadcn anatomy：Dialog/AlertDialog/Sheet 的
> Trigger/Portal/Overlay/Content/Header/Footer/Title/Description/Close，Popover Trigger/Anchor/Content，Select
> Trigger/Content/Group/Item/Indicator/ScrollButtons，Tabs Root/List/Trigger/Content，Accordion
> Root/Item/Trigger/Content，NavigationMenu/Combobox/Menu 等。Fret 还把 recipe parts 和 primitive facades 分层，适合映射到 GPUI
> Element/Entity 或 open-gpui 的 typed part 模型。

#### `state_ownership_model`

> 采用分层状态所有权：应用 View 使用 `LocalState` 和 `Model<T>`；基础 primitive 倡导 value-first、caller-owned handles/callbacks/typed
> actions；组件提供 controlled/uncontrolled 形态（如 accordion/toggle_group/tabs 等的 model/defaultValue）；selector/query 是 optional
> adapter，不进入 primitive 基础 API；runtime 只提供 action hook 机制，不直接写组件状态。

### Headless 与行为

#### `headless_boundary`

> 边界非常清楚：`fret-ui` 只保留 routing、focus/capture、layout、semantics、overlay substrate、placement、scroll/virtualization、text/IME
> 等机制；`fret-ui-headless`/`fret-ui-kit::headless` 放确定性状态机和算法；`fret-ui-kit::primitives` 给 Radix 命名 facade；`fret-ui-shadcn`
> 提供视觉 recipe 与 taxonomy。这个边界是 open-gpui 最值得借鉴的部分。

#### `accessibility_model`

> 以 renderer-independent Semantics Tree 为核心，包含稳定
> identity、role/state/action、label/value、bounds、关系（labelled_by/described_by/controls）和 overlay/modal reachability；native
> 通过 AccessKit bridge 映射。组件层负责 stamping roles/labels/states；diagnostics/scripts 优先使用 semantics/test-id selector。需要注意
> AccessKit/winit 兼容状态仍受平台生态影响。

#### `positioning_and_collision_model`

> Overlay placement 是 `fret-ui` 纯函数合同，输入 outer/anchor/content/side_offset/preferred_side/align，保证同坐标系、同窗口、确定性；算法借鉴
> Floating UI 的 preferred/flip/shift/clamp/best-fit，已有 sized helper 和当前代码导出 arrow/collision/shift/offset 类型。Dismiss/focus
> restore/nested overlay/hover intent 在组件或 kit 层处理。

#### `interaction_state_machines`

> 存在可单测的等价状态机层：roving_focus、typeahead、menu_nav、hover_intent、presence、focus_scope、dismissible_layer、cmdk_score/selection、safe_hover、tooltip_delay_group、slider、table/scroll_area
> 等。ADR 明确 runtime action hooks 只是机制，APG/Radix/cmdk policy 在 headless/kit/shadcn 层测试。

### 渲染与性能

#### `rendering_model`

> 非 DOM/WebView；声明式每帧构建 Element tree，挂载到 retained `UiTree`，经 layout/prepaint/paint 生成 `Scene` display list，由 wgpu/GPU-
> first renderer 提交；支持 native desktop 与 wasm/WebGPU path。

#### `performance_model`

> 性能策略是多层闭环：event-driven scheduling、dirty propagation、ViewBoundary、prepaint、SceneFragment/paint replay、renderer
> plan/encoding reuse、virtual_list Fenwick/overscan/range、scroll handle、text/glyph cache、diagnostics perf counters 和 UI
> Gallery perf scripts。大表格/树/代码编辑器优先靠虚拟化、边界隔离、缓存和 GPU 场景复用，而不是 DOM diff。

#### `native_advantage`

> native GPUI/open-gpui 应优先在 editor-grade 场景胜出：大文本/代码编辑器、超大列表/表格/树、dock/tear-off/multi-window、嵌入 GPU
> viewport/canvas、低延迟输入、精确 IME/selection、可控渲染管线、诊断可复现包。Fret 的 code-editor resize 与 boundary/cache workstream
> 说明性能差异化应来自运行时边界和渲染管线，不只是组件外观。

#### `web_ecosystem_advantage`

> Web/Tauri/Electron 仍天然强在 DOM/CSS 布局、浏览器 a11y、shadcn/npm copy-to-own registry、DevTools、海量第三方组件、CSS 动画与设计 token 工具链。open-
> gpui 不应早期追逐完整 Web 组件市场或像素级 CSS parity，而应参考其契约并在必要时提供 WebView/asset/interop 通道。

### 主题与设计系统

#### `theme_token_model`

> Fret 有 typed baseline key（颜色、metric）和 string token
> 扩展（Color/Metric/Corners/Number/Duration/Easing/TextStyle），ThemeConfig/ThemeSnapshot 支持运行时读取、fallback 和 missing-token
> diagnostics；shadcn new-york-v4 主题 JSON 按 base color 存放；workspace themes 可作为 parity/token coverage 输入。状态/变体由 recipe 的
> ChromeRefinement、WidgetState、variant/size props 和 token resolver 组合。

#### `style_customization_boundary`

> runtime 提供 token/config/snapshot 与低层 paint/layout 机制；`fret-ui-kit` 提供 token-aware style/layout refinement 和 `.ui()`
> authoring；`fret-ui-shadcn` recipe 决定默认 chrome、variants、sizes、states；应用通过 theme install、token
> override、refine_style/refine_layout、raw parts 和 typed children 改外观。这样避免把 shadcn 默认样式塞进 runtime。

### 组件表面

#### `component_coverage`

> 覆盖非常宽：基础控件、form、overlay、navigation、data display、feedback、application shell、rich/AI/editor-grade surfaces 都有。核心 shadcn
> 表面包括
> button/input/textarea/checkbox/switch/radio/slider/tabs/accordion/dialog/popover/select/combobox/menus/table/data_table/sidebar/sonner
> 等；gallery-dev 还覆盖 code editor、data grid、AI widgets、Material3、performance torture pages。

#### `must_have_for_open_gpui`

> 必须借鉴三类能力：1. 机制/策略边界和 headless 状态机层；2. shadcn/Radix-like component anatomy 与 typed facade/raw escape hatch；3.
> docs/gallery/diagnostics/source-policy gates。对 open-gpui 而言，P0 应先补
> Button/Input/Dialog/Popover/Select/Tabs/Table/VirtualList/Tooltip/Menu/Toast 的 contract + examples + tests，而不是一次性追全部
> gallery。

#### `do_not_chase`

> 当前阶段不应追 Fret 的全部产品外延：AI widget zoo、Material3 全量、Magic/material effects、workspace shell、docking/viewport/gizmo、code
> editor、plot/chart、asset/icon marketplace、诊断 CLI 全家桶。也不应照搬 shadcn copy-to-own registry 或 Web CSS/Tailwind implementation
> details；先把 primitive contracts、a11y、overlay、theme、gallery gates 做稳。

### 文档测试工具

#### `docs_gallery_model`

> Gallery 是组件 discovery + conformance surface，docs 页面不嵌 Rust 字符串而引用 snippet 源码，测试要求 Preview ≡ Code；PageSpec
> 提供可路由、可启动、可搜索的页面索引；每个组件有 docs surface tests 和 diag scripts。尚未完全单一事实源，但已经接近“源码 snippet + gallery + tests + diag
> script”的事实链。

#### `testing_strategy`

> 多层测试：unit/contract tests（headless、runtime、overlay solver、virtualization）、component parity/web-vs-Fret/layout/chrome
> tests、gallery docs surface/source-policy tests、diagnostics JSON scripts/suites、semantics/test-id gates、perf surface
> tests、view-cache/overlay synthesis matrix、renderer conformance。Rust 推荐 nextest；诊断脚本提供 E2E 交互验证。

#### `diagnostics_and_failure_quality`

> 非常强：bundle.schema2、meta/index/test-id sidecars、script.result reason_code、AI packet、layout sidecar、semantics
> selectors、resource/perf/renderer triage hints、diag query/slice/compare/matrix。失败可定位到组件、test_id、semantics
> role、bounds、layer/hit-test、token/resource/perf 线索，适合 AI 自动读包修复。

### 治理

#### `maintenance_cost`

> 成本高：需要维护 runtime 机制、headless 状态机、shadcn parity、gallery、web goldens、diagnostics CLI、theme/token
> coverage、AccessKit/wasm/native 后端和大量 ADR。收益是边界清晰、可自动化验证；但如果团队规模小，open-gpui 应裁剪为核心 contract + 少量高价值组件，不应复制完整 Fret 范围。

#### `risks`

> 主要风险是实验性与 AI 生成质量、过度复刻 Web/shadcn、runtime 与 component policy 再次耦合、组件表面过宽导致维护失控、主题/token schema 漂移、a11y/platform bridge
> 不完整、diagnostics 体系复杂、第三方生态还未证明、性能优势被厚重 recipe 和 gallery 包袱稀释。

#### `open_gpui_relevance`

> 建议：reference-only + trial。Fret 不应被 open-gpui 直接 adopt，但应作为本地高价值设计参考：采纳机制/策略分层、headless primitive、typed
> anatomy、gallery/diagnostics gates、Cargo+feature+scaffold 分发；试点实现少量 open-gpui 原生组件并用 Fret 风格的 contract tests 验证。避免照搬完整生态和
> AI/widget 扩张面。

### 不确定字段（已跳过）

- `design_token_pipeline`
- `registry_viability`
- `third_party_ecosystem_path`
- `versioning_and_breakage`

## <a id="gpui-component"></a>10. gpui-component

- 结果文件：`gpui_component.json`
- 调研类别：`local_reference_gpui_component_suite`
- 纳入原因：
  完整 GPUI 原生组件库，覆盖 story/docs/theme/assets/webview 等；用于判断 open-gpui 不该盲目追哪些应用级能力。

### 定位

#### `positioning`

> gpui-component 的定位是基于 GPUI 的完整原生桌面组件库，同时包含主题系统、资产/图标桥接、桌面 story/gallery、VitePress 文档、WASM story-web、webview
> 示例和应用级外壳能力。它不是纯 headless primitive，也不是只提供 token pipeline 或 registry 的项目，更接近“应用开发可直接采用的 GPUI 原生组件套件”。

#### `target_users`

> 主要服务 GPUI 桌面应用开发者、希望快速搭建原生 Rust 桌面产品的产品团队，以及需要参考完整组件实现的 open-gpui 框架/组件作者。它对设计系统作者也有价值，但缺少独立的 headless
> 合约和机器可读组件清单，因此不是专门面向 AI agent 或第三方 registry 维护者设计。

#### `primary_value_proposition`

> 核心价值是把 GPUI 的低层渲染能力包装成 60 个以上可直接使用的桌面 UI 组件，覆盖按钮、表单、overlay、列表、表格、树、图表、编辑器、dock、主题和 story。与 open-gpui
> 的原生性能目标高度相关，但它的价值重心是“可用组件面”和应用能力，不应被 open-gpui 盲目等同为底层通用框架目标。

### 分发与生态

#### `distribution_model`

> 主要分发方式是 Cargo 包依赖和 workspace 本地路径依赖：`gpui-component` 作为 `crates/ui` crate 发布，`gpui-component-
> assets`、`macros`、`story`、`story-web` 等作为配套 crate 或工具存在。README 和文档强调在 `Cargo.toml` 中添加依赖并调用
> `gpui_component::init(cx)`，没有看到类似 shadcn 的“复制源码到项目”的命令，也没有组件 registry、CLI add、模板市场或插件市场。feature flags 主要用于可选能力，例如
> decimal、inspector 和大量 Tree-sitter 语言包。

#### `source_ownership`

> 使用者通常通过 crate API 消费组件，源码所有权属于上游 crate；若本地以 path 依赖或 fork 方式使用，可以直接 patch，但升级需要承担 Rust crate API 变更、GPUI
> 版本绑定和主题/资产生成逻辑的合并成本。相比 copy-to-own，常规依赖升级简单；相比 registry recipe，深度定制结构和行为时更容易与上游分叉。

#### `rust_distribution_fit`

> 与 Rust 分发生态适配度较高：使用 workspace、crates.io 元数据、SemVer 版本号、feature flags、build.rs、proc macro、assets crate 和常规 Cargo
> 依赖。它也支持本地 path 依赖来跟 open-gpui 同步开发。短板是没有 cargo-generate/xtask/CLI scaffold 流程，组件添加和迁移更依赖文档阅读与手写代码。

### AI 时代设计

#### `ai_friendliness`

> 对 AI 的友好度中等偏高：Rust 模块拆分清楚，组件名、story 文件、docs 页面和 README 示例容易检索，builder API 也便于组合。限制是组件事实源分散在源码、story 和 Markdown 中，没有统一
> manifest 描述 props、variants、state、a11y、示例和测试；AI 修改后很难自动判断是否破坏交互、视觉或主题边界。

#### `machine_readable_contracts`

> 机器可读契约集中在主题和部分数据结构：`Theme`、`ThemeConfig`、`ThemeColor`、高亮主题等派生 `Serialize/Deserialize/JsonSchema`，主题文件带 `.theme-
> schema.json`，图标枚举通过 assets crate 和 proc macro 生成。组件级契约没有看到统一 JSON/YAML manifest，也没有能直接驱动 docs、gallery、scaffold 和测试的
> typed registry。

#### `copy_modify_verify_loop`

> 官方路径是依赖 crate、阅读 docs/story、在本地应用中组合组件并运行 Cargo 测试或 story gallery；没有 copy-to-own 组件生成流程。若开发者复制组件源码修改，验证主要依赖 `cargo
> test`、运行 story、手工检查 gallery 和局部单测；缺少统一的 contract、visual、a11y、性能门禁来支持 AI 自动改完即验。

### API 与组合

#### `api_ergonomics`

> API 以 Rust builder pattern、`IntoElement`/`RenderOnce`、`Entity<State>`、trait 扩展和回调为主，符合 GPUI 的元素模型。简单组件如
> Button、Badge、Alert 使用链式 variant/size/style 方法；复杂组件如 Input、List、DataTable、Tree 使用 state/delegate/entity；overlay 组件暴露
> trigger、content、open、on_open_change、overlay_closable 等组合点。整体上手成本低于裸 GPUI，但复杂组件仍要求理解 GPUI 生命周期、focus handle 和 context。

#### `customization_model`

> 定制模型分为几层：全局 Theme/ThemeColor 控制颜色、字体、圆角、阴影和部分组件设置；组件 props 控制 size、variant、checked、disabled、overlay、placement
> 等行为；`Styled`/`StyleRefinement` 和 child composition 提供结构/样式 escape hatch；高阶组件可以通过 state/delegate
> 改行为。优势是灵活，风险是样式、行为和应用状态边界没有统一 contract 约束。

#### `component_anatomy_model`

> 复杂组件有一定 anatomy，但不是 Radix 式稳定 parts API。Dialog 拆出 DialogContent、DialogTitle、header/footer/content builder；Table 拆出
> TableHeader、TableBody、TableRow、TableCell；Menu 有 PopupMenu、PopupMenuItem、submenu；Sidebar、Dock、Stepper、Form
> 也有子结构。Popover、Tooltip、Select、Combobox 更偏组件封装和 state/entity 组合，没有系统性命名为 root/trigger/content/item/indicator/portal 的公共
> parts 合约。

#### `state_ownership_model`

> 状态所有权是混合模型：多数视觉组件是 stateless `RenderOnce`，应用通过 props/callback 持有状态；部分交互组件通过 `window.use_keyed_state` 或 `Entity<State>`
> 持有内部状态；Popover 支持 `default_open`、受控 `open` 和 `on_open_change`；Input/List/Table/Tree 通过 State/Delegate hoist 一部分业务状态；Root
> 统一管理 dialog、sheet、notification、tooltip、window text selection 和 focus restore。该模型符合 GPUI，但还不是 renderer-neutral 的独立状态机层。

### Headless 与行为

#### `headless_boundary`

> headless 边界不清晰。行为、状态、布局、主题和渲染通常写在同一个组件模块中，例如 Popover 同时处理 open 状态、focus、dismiss、anchor 渲染和样式；DataTable
> 同时绑定键盘动作、状态和渲染；Dialog 同时管理 overlay、动画、focus trap 和按钮。对 open-gpui 的启示是可以参考行为细节，但应把可测试行为合约、定位算法、a11y metadata、render
> adapter 和主题 recipe 进一步分层。

#### `positioning_and_collision_model`

> overlay 定位能力存在但不完整。Popover 使用 GPUI `anchored()`、`Anchor`、trigger bounds、`snap_to_window_with_margin` 和 outside click
> dismiss；Tooltip 有自定义 above/below 选择、viewport clamp、延迟、grace period 和切换动画；PopupMenu 子菜单会根据窗口宽高选择左右/上下 anchor。没有看到类似
> Floating UI 的 middleware 化 flip/shift/size/arrow/safe polygon/focus return 统一抽象，因此 open-gpui 应抽取几何 contract
> 而不是直接照搬每个组件的局部实现。

#### `interaction_state_machines`

> 没有看到显式 finite state machine 框架，更多是 Rust 字段、回调、actions 和 focus/dismiss subscription 组成的等价状态逻辑。菜单选择、Popover
> open/close、Tooltip show/hide epoch、Dialog layer、Table selection、Text selection 都有可测逻辑，但契约分散在组件内部。对 open-gpui 来说，应把
> menu/select/dialog/combobox/table/tree 等核心交互沉淀为可测试状态表或事件合约。

### 渲染与性能

#### `rendering_model`

> 渲染模型是 GPUI 原生 retained/entity 元素树加自绘/GPU 管线能力，组件通过
> `Render`、`RenderOnce`、`IntoElement`、`Element`、`deferred`、`anchored`、layout/prepaint/paint 等机制渲染；另有 WASM story-web 和独立
> webview crate/示例，但核心组件不是 DOM/WebView 实现。

#### `native_advantage`

> native GPUI 的优势最明显体现在高密度桌面场景：大表格/大列表/树、代码编辑器、文本选择、dock/panel 布局、原生窗口标题栏/边框、低延迟 overlay 和自绘图表。这些能力能形成 open-gpui 相对
> WebView/DOM 的差异化，尤其适合金融终端、开发工具、监控台和复杂桌面生产力软件。

#### `web_ecosystem_advantage`

> Web/Tauri/Electron 生态在富文本编辑生态、浏览器可访问性、CSS 生态、组件市场、设计 token 工具链、图表/可视化生态、国际化和大量第三方控件上仍更强。gpui-component 自带
> webview、VitePress 文档和 WASM gallery，说明它也承认 Web 生态价值。open-gpui 当前不应追完整 Web 组件宇宙，应优先做原生高密度组件、与 WebView 互操作，并保留 Web
> 文档/gallery 工具链。

### 主题与设计系统

#### `theme_token_model`

> 主题模型比较完整：`Theme` 持有颜色、字体、字号、圆角、阴影、滚动条、通知、sheet、list、tile 等运行时配置；`ThemeColor` 覆盖基础色、语义色、组件色和状态色；`ThemeConfig`/`ThemeSet`
> 通过 JSON 加载，支持 light/dark mode、fallback、Zed 风格 highlight theme 和主题目录热加载。它是实用的运行时 token/schema 模型。

#### `design_token_pipeline`

> 没有看到 DTCG、Style Dictionary 或 Tailwind-like transform 的跨平台 token pipeline。已有能力是 Rust schema 派生、`.theme-schema.json`、主题
> JSON 文件、默认主题和运行时 registry。对 open-gpui 可以参考 schema + runtime fallback，但若要服务多平台设计系统，需要额外补 token transform、schema drift
> gate、token lint 和生成文档/示例的流水线。

#### `style_customization_boundary`

> 样式责任边界是“框架默认主题 + 组件内 recipe + 组件 props + 用户 style refinement/children”共同承担。framework 提供 Theme 和
> StyledExt，组件内部决定变体、尺寸和状态样式，用户可以通过 props、`Styled`、自定义 child 和 fork 逃逸。这个边界务实但偏松，open-gpui 若要长期稳定，应明确哪些样式是 design
> token，哪些是 recipe，哪些只能由应用源码覆盖。

### 组件表面

#### `component_coverage`

> 覆盖度很广：基础控件、表单、overlay、导航、数据展示、反馈、应用外壳、图表、文本/Markdown/HTML、代码编辑器、dock、settings、title bar、sidebar、table/tree/virtual list
> 等都有实现或 story。它已经超出通用组件库，进入桌面应用套件和产品框架雏形。

#### `must_have_for_open_gpui`

> open-gpui 必须优先补齐的是底层通用能力：Root/layer 管理、Dialog/Sheet/Popover/Tooltip/Menu 的 overlay/focus/dismiss
> 基元，Button/Input/Checkbox/Radio/Switch/Select/Combobox 的基础表单控件，VirtualList/Table/Tree 的高密度数据
> primitive，Theme/ThemeColor/schema/fallback，FocusTrap 和键盘动作模型，以及 story/gallery 作为 dogfood 面。完整编辑器、dock、chart、settings
> 可以作为后续试点或参考。

#### `do_not_chase`

> 当前阶段不应盲目追应用级能力：完整代码编辑器与 LSP、Dock/Tiles 工作区系统、Settings 页面框架、TitleBar/WindowBorder 的产品级封装、WebView
> crate、系统监控示例、完整图表库、Markdown/HTML 渲染器以及大量内置主题数量。它们对成品应用有价值，但会稀释 open-gpui 作为通用 UI framework 的边界，应先沉淀可复用 primitive 和验证体系。

### 文档测试工具

#### `docs_gallery_model`

> docs、story 和 gallery 不是同一事实源派生。Rust `story` crate 手写 story 列表和 Story trait，VitePress 文档手写 Markdown 页面，README 和 examples
> 也独立维护；WASM story-web 复用 story 能力但仍是单独 crate。优点是可读和可演示，缺点是组件源码、文档、示例、story、测试之间容易漂移，AI 也难以自动知道哪个才是权威。

#### `testing_strategy`

> 测试覆盖存在但分散：本次检索到 crates 下约 220 个 `#[test]` 或 `#[gpui::test]`，包含 builder、算法、tooltip 定位、popover、文本选择、rope、highlight、plot
> 等；GPUI test support 被用于部分交互测试。没有看到统一 visual snapshot、a11y、性能、API surface、schema drift、import boundary gate。对 open-gpui
> 应把这些门禁产品化，而不是只靠 story 手工检查。

### 治理

#### `maintenance_cost`

> 维护成本高。组件面广意味着每次 GPUI、主题、focus、layout、输入法、文本、Tree-sitter、WASM、平台窗口行为变化都可能影响多个模块；story/docs/tests
> 需要同步维护；高级组件如编辑器、表格、dock、图表和 overlay 需要长期专业投入。对 open-gpui 来说，应先收敛核心 primitive 和验证基础设施，再逐步扩大组件面。

#### `risks`

> 主要风险包括：把应用套件误当成框架底座，导致 open-gpui 过早背负编辑器、dock、chart、settings 等应用级复杂度；行为逻辑和渲染样式耦合，后续难以抽 headless；a11y
> 语义不足；docs/story/source 漂移；主题 token 缺少标准 pipeline；Web 生态能力复刻过多导致原生优势被稀释；第三方组件如果没有 contract 和测试门禁会碎片化。

#### `open_gpui_relevance`

> 最终建议是仅参考，局部试点采用。open-gpui 应参考 gpui-component 的组件覆盖、Root layer、主题 registry、VirtualList/DataTable、focus trap、overlay 细节和
> story dogfood，但不要直接把它的全部应用级能力设为路线图。直接设计含义是：先设计深层 primitive、机器可读契约、验证门禁和主题 schema，再让组件库在这些基础上生长。

### 不确定字段（已跳过）

- `accessibility_model`
- `diagnostics_and_failure_quality`
- `performance_model`
- `registry_viability`
- `third_party_ecosystem_path`
- `versioning_and_breakage`

## <a id="zed-ui-gpui"></a>11. Zed UI / GPUI

- 结果文件：`Zed_UI_GPUI.json`
- 调研类别：`production_gpui_reference`
- 纳入原因：
  生产级 GPUI UI 的真实架构边界；应重点看哪些能力属于编辑器产品，哪些可沉淀为通用 framework。

### 定位

#### `positioning`

> Zed UI / GPUI 的定位是生产级桌面应用内部 UI primitive 与组件层：它建立在 GPUI Element、Entity、Window、App、theme、icons、menu 等 Zed 内部 crate
> 之上，覆盖按钮、标签、列表、菜单、弹层、表格、主题样式、组件预览和若干编辑器/协作/AI 产品组件。它不是独立发布的通用框架，也不是纯 headless primitive。

#### `target_users`

> 主要服务 Zed 编辑器团队和 Zed monorepo 内部功能团队；次级服务对象是需要参考真实 GPUI 生产 UI 边界的框架作者。对 open-gpui 来说，它更像架构样本和经验库，而不是可以直接复用的外部组件生态。

#### `primary_value_proposition`

> 核心价值是把 GPUI 的低层 Element API 包装成一致、可组合、可预览、可主题化的生产组件，同时保留 GPUI 原生渲染、焦点、快捷键、滚动和弹层能力。与 open-gpui 高度匹配的是 builder
> API、Component preview、语义 token、AccessKit/role 集成、虚拟列表表格和 overlay 生命周期；不匹配的是强绑定 Zed 编辑器业务、内部 workspace 和 GPL/monorepo
> 分发模式。

### 分发与生态

#### `distribution_model`

> 分发方式是 Zed monorepo 内部 Cargo workspace 依赖。`ui` crate 的 `publish.workspace` 表明它不是面向 crates.io 的独立外部分发包；组件通过 Rust 模块导出和
> `ui::prelude` 使用，组件预览通过 `RegisterComponent` 派生宏、`inventory` 注册和 `component_preview` 工作区面板组织。没有看到 shadcn 式远程 registry、CLI
> add、copy-to-own、模板脚手架或第三方 marketplace。

#### `source_ownership`

> 源码由 Zed 项目拥有，内部团队可以直接修改组件、样式和行为，升级成本由同一 monorepo 的同步重构承担。外部使用者若参考该代码，更接近 fork 或源码学习，不能获得稳定 SemVer
> API、迁移指南或可低成本合并的组件包升级路径。对 open-gpui 的启发是：内部源码所有权能支撑快速产品迭代，但通用框架需要额外定义 public API、兼容策略和分发边界。

#### `rust_distribution_fit`

> Zed UI 与 Rust/Cargo 的适配是自然的：组件就是 crate 模块，依赖和 feature graph 由 workspace 统一管理，类型系统约束 builder API 和 Element 组合，测试用 `gpui`
> test support。缺口在于它没有面向外部的 crates.io SemVer、cargo add、cargo generate、xtask scaffold 或 feature flags 组件矩阵。open-gpui 可把 Zed
> 的 crate 内聚方式作为核心包结构，再补 CLI 与 metadata 层。

### AI 时代设计

#### `ai_friendliness`

> 代码整体对 AI 读取较友好：组件文件粒度清晰，builder 方法名语义明确，doc comment 和 preview 示例较多，`Component` trait 提供
> name、description、scope、status、preview 元数据，`component_preview` 能按 scope 搜索展示组件。限制是缺少独立 schema、manifest、契约测试清单和稳定导入边界，AI
> 可以理解源码，但不一定能自动判断修改后的视觉、a11y 或交互是否仍正确。

#### `machine_readable_contracts`

> 已有一部分机器可读契约：`ComponentMetadata` 记录 id、name、description、scope、sort_name、status、preview 函数；`Color` 等类型通过
> `Documented`、`DocumentedFields`、`DocumentedVariants` 暴露文档；Rust 类型系统约束 props、状态和事件。它没有 JSON/YAML registry schema，也没有把
> anatomy、AccessKit 节点、键盘表、性能预算、截图基线统一声明为可外部消费的 manifest。

### API 与组合

#### `api_ergonomics`

> API 体验以 Rust builder pattern 和 GPUI Element 组合为主，例如
> `Button::new(...).start_icon(...).toggle_state(...).on_click(...)`、`DropdownMenu::new(...).style(...).full_width(...)`、`Table::new(cols).header(...).row(...).uniform_list(...)`。常用能力通过
> traits 统一，如 `Clickable`、`Disableable`、`Toggleable`、`FixedWidth`、`ButtonCommon`、`StyledTypography`。这非常适合
> Rust/GPUI，因为类型、所有权和闭包事件都自然落在编译期。

#### `customization_model`

> 定制模型分为几层：theme crate 提供颜色、字体、密度和状态 token；`DynamicSpacing`、`Color`、`ElevationIndex`、`TextSize` 提供语义样式；组件 builder prop
> 提供有限变体和行为开关；`ButtonLike`、`LabelLike`、`ListItem` 等低层组合件提供 escape hatch；最终还可以直接写 GPUI `div()` 和 Element。缺点是样式与 Zed theme
> settings 强绑定，缺少外部 theme schema 和跨项目 token pipeline。

#### `component_anatomy_model`

> 复杂组件有局部 anatomy，但不是 Radix 式严格 Root/Trigger/Content/Item/Portal 模型。按钮拆出 `ButtonLike` 作为底座，列表有 start_slot/end_slot，Modal 有
> Header/Section/Footer，PopoverMenu 有 trigger/menu/handle，ContextMenu 有 item/header/separator/submenu，Table 有
> interaction_state、column config、row renderer。适合 open-gpui 借鉴的部分是将底座、slot、handle、render state 和 preview 分开，但应进一步规范
> anatomy 命名和行为层边界。

#### `state_ownership_model`

> 状态所有权是 GPUI 风格的混合模型：简单视觉状态通常由 builder 输入，如 selected、disabled、expanded、toggle_state；复杂运行时状态由 `Entity` 承载，如
> `ContextMenu`、`TableInteractionState`、`ResizableColumnsState`、`RedistributableColumnsState`；瞬态 Element 状态通过
> `window.with_element_state` 保存，如弹层菜单句柄、触发器 bounds 和右键菜单位置；应用业务状态由调用方闭包和外部 Entity 拥有。这比 Web controlled/uncontrolled
> 更显式，但尚未抽象为 renderer-neutral state contract。

### Headless 与行为

#### `headless_boundary`

> headless 边界不够硬。`ContextMenu` 同时处理 item 数据、焦点、键盘 action、submenu 状态、dismiss、定位、a11y role 和具体渲染；`PopoverMenu` 同时处理
> trigger、menu Entity、anchored layout、focus return 和绘制时事件；`ButtonLike` 同时处理样式、角色、事件、tooltip 和 a11y action。Zed
> 的做法适合产品内聚和快速迭代，open-gpui 通用框架则应把行为状态机、定位服务、AccessKit metadata、render adapter 和 theme recipe 拆得更清楚。

#### `accessibility_model`

> 可访问性已经进入组件 API，但覆盖不均。Button/ButtonLike 支持 role、aria_label、aria_expanded、aria_toggled、AccessKit action；DropdownMenu 用
> ComboBox role 和 Expand/Collapse action；ContextMenu 用 Menu/MenuItem、active descendant、键盘 action 和 focus
> return；TreeViewItem 用 TreeItem、level、selected、expanded；Switch 用 Switch role 和 toggled。缺口是 Checkbox 等部分组件仍偏视觉实现，未见统一 a11y
> contract tests 或 AccessKit 树快照门禁。

#### `positioning_and_collision_model`

> 定位模型主要依赖 GPUI 的 `anchored()`、`deferred()`、`snap_to_window_with_margin(px(8.))`、Anchor、trigger bounds、mouse position 和
> occlusion。PopoverMenu 支持 anchor、attach、offset、full_width、focus return 和外部点击关闭；RightClickMenu 支持鼠标位置或元素角点触发；ContextMenu
> 自己维护 submenu offset、flip_left、hover target、安全区域和 blur 忽略窗口。它有生产经验，但不是独立几何算法库，缺少 Floating UI 式 flip/shift/size/arrow/safe
> polygon 的统一可测试 contract。

#### `interaction_state_machines`

> 交互状态机是隐式的 Rust 状态与 action handler，而不是显式 finite state machine。ContextMenu 有
> `SubmenuState`、`HoverTarget`、selected_index、delayed、clicked、keep_open_on_confirm、ignore_blur_until 等状态，并绑定
> SelectFirst/Next/Previous/Confirm/Cancel 等 action；PopoverMenu 有 show/hide/toggle handle；Table 有列宽拖拽状态。优点是贴合产品，缺点是 AI
> 和第三方很难从源码外部验证所有状态转移。

### 渲染与性能

#### `rendering_model`

> 渲染模型是 GPUI 原生 Element/Entity 模型：Rust 构建 Element tree，组件实现 `RenderOnce` 或 `Render`，通过 GPUI
> layout、paint、prepaint、deferred、canvas、uniform_list/list、Window/App 上下文和 GPU/native 绘制运行。它不是 DOM/WebView，也不是 egui 式
> immediate mode；更接近 retained/native declarative composition 加显式运行时 handle。

#### `performance_model`

> 性能策略体现了生产桌面 UI 的重点：大表格可用 `uniform_list` 虚拟化或 `list` 支持可变行高；表格滚动状态和水平滚动由 `TableInteractionState` 持有；列宽 resize
> 有独立状态和算法单测；弹层用 deferred 绘制避免普通布局流干扰；组件广泛使用 GPUI 原生元素和 theme token，避免 WebView 开销。缺口是没有看到统一性能 budget、profiling hooks
> 或大规模组件基准体系。

#### `native_advantage`

> Zed UI 展示的 native GPUI 优势集中在高密度桌面应用：低延迟菜单和快捷键、复杂焦点恢复、原生可访问性桥接、长列表和表格虚拟化、可变高度列表、编辑器字体和 UI 字体区分、跨平台窗口事件、右键菜单、拖拽列宽和细粒度
> repaint。open-gpui 应优先放大这些优势，而不是先追 Web 营销组件或轻量表单生态。

#### `web_ecosystem_advantage`

> Web/Tauri/Electron 仍然在通用组件数量、表单生态、图表、CSS token 工具、Storybook/Chromatic、浏览器 a11y 调试、远程 registry、主题市场和文档站生成上更成熟。Zed UI
> 的组件预览和内部 registry 证明 native 可以补一部分，但 open-gpui 不应正面复刻完整 Web 生态，应优先做原生桌面差异化并允许 WebView/外部工具互操作。

### 主题与设计系统

#### `theme_token_model`

> 主题模型以 Zed 的 `theme` crate 为中心：`ActiveTheme` 提供 colors、status、styles、appearance、theme_settings；`Color` 是语义枚举并映射到具体
> HSLA；`DynamicSpacing` 根据 Compact/Default/Comfortable density 派生 rem/px；`TextSize` 区分
> UI、Editor、Small、Large；`ElevationIndex` 定义 surface/modal 等背景和 shadow。它适合产品一致性，但外部 token schema、fallback、跨 app theme file
> 和 DTCG 映射并未在 `ui` crate 中体现。

#### `style_customization_boundary`

> 当前边界是 framework/component 层提供默认视觉，theme crate 负责全局颜色和字体，组件 prop 控制少量变体，用户通过组合低层 `div()`、`ButtonLike`、slot 和自定义 Element 做
> escape hatch。对于 Zed 内部这足够高效；对 open-gpui 应进一步约定 core primitive 不绑定产品色彩，官方 theme recipe 提供默认风格，业务组件可覆盖结构和行为，app adapter
> 处理平台差异。

### 组件表面

#### `component_coverage`

> 覆盖中等偏生产工具型：基础控件有 Button、IconButton、ToggleButton、Checkbox、Switch、DropdownMenu；展示与反馈有
> Label、Headline、Icon、Avatar、Banner、Callout、Tooltip、Notification、Progress、Indicator、CountBadge；布局和导航有
> List、ListItem、TreeViewItem、Tab、TabBar、Stack、Divider、Scrollbar、Modal、Popover、ContextMenu、RightClickMenu；数据展示有
> Table、DiffStat、KeyBinding；还包含 AI、协作、project empty state 等 Zed 产品组件。缺少通用输入域、复杂 form、date picker、combobox、完整 rich data
> grid、图表等外部应用常见组件。

#### `must_have_for_open_gpui`

> 必须吸收的是：`prelude` 和 traits 统一 API；强类型 builder 组件；ButtonLike/ListItem 这类可组合底座；Component trait、metadata、preview
> gallery；语义颜色、密度间距、字体和 elevation token；Popover/ContextMenu 的 focus return、dismiss、deferred overlay；Table
> 的虚拟列表、列宽状态和算法测试；AccessKit role/action 在组件 API 中前置。这些是 native GPUI 通用 UI 框架的核心骨架。

#### `do_not_chase`

> 不要追 Zed 编辑器产品专属能力：AI agent 设置卡片、thread item、协作通知、facepile、project empty state、版本控制语义颜色过度细分、workspace item 序列化、Zed
> command/action 命名、编辑器 tab 细节和内部 theme settings 形状。也不要把 GPL monorepo 内部分发、隐式状态机和强产品耦合照搬为 open-gpui 公共 API。

### 文档测试工具

#### `docs_gallery_model`

> Zed 有内建组件预览模型：组件实现 `Component`，提供 scope、status、description、preview；`RegisterComponent` 通过 inventory
> 收集；`component_preview` 提供搜索、按 scope 分组、单组件页面和所有组件页面。这比普通 examples 更接近 native Storybook。缺口是 docs、schema、截图测试、a11y
> contract 和 scaffold 没有统一事实源；open-gpui 可把这一层升级为可机器读取的 gallery/contract 系统。

#### `testing_strategy`

> 现有测试策略是局部的：`data_table` 对列宽重分配、拖拽、pin layout 等算法有单元测试；`ContextMenu` 有键盘导航跳过 header/separator 的 GPUI 测试；Cargo dev
> dependency 打开 `gpui` test-support。未见统一组件截图回归、交互录制、AccessKit tree assertion、性能基准、public API drift 或 import boundary
> gate。open-gpui 应把这些补为框架级测试矩阵。

### 治理

#### `maintenance_cost`

> 维护成本偏高但集中可控。Zed 团队需要同时维护组件源码、theme 映射、preview、a11y、焦点、快捷键、overlay、虚拟列表和产品专属组件；好处是所有消费方在同一仓库，重构和 dogfood 速度快。open-gpui
> 若要通用化，成本会更高，因为还要承担外部文档、稳定 API、生态分发、兼容测试、第三方质量和跨项目主题需求。

#### `risks`

> 主要风险是把 Zed 的产品内聚设计误读为通用框架设计：内部 crate 可破坏性重构、业务组件和 primitive 混放、隐式状态机、theme 强绑定、组件 preview 非 schema 化、overlay
> 几何分散在组件内。另一个风险是 GPL/monorepo 代码边界和 Zed 私有业务语义不适合作为 open-gpui 直接依赖。若 open-gpui 只模仿组件表面，会得到一个难治理的原生 widget 集合，而不是可扩展
> framework。

#### `open_gpui_relevance`

> 建议为 reference-only 加 targeted adopt：不要直接采用 Zed UI 作为 open-gpui 组件库，但应定向吸收其生产经验。直接设计含义是：建立 GPUI-native builder API 和
> prelude；抽象 Component metadata/gallery；定义强类型 theme token；把 overlay/focus/dismiss 做成独立 primitive；把 table/list 虚拟化与列宽状态做成
> contract；把 a11y role/action 纳入组件公共 API；明确区分 framework core、通用组件、产品 recipe 和应用级业务组件。

### 不确定字段（已跳过）

- `copy_modify_verify_loop`
- `design_token_pipeline`
- `diagnostics_and_failure_quality`
- `registry_viability`
- `third_party_ecosystem_path`
- `versioning_and_breakage`

## <a id="swiftui"></a>12. SwiftUI

- 结果文件：`SwiftUI.json`
- 调研类别：`native_declarative_ui`
- 纳入原因：
  声明式 native UI、state/data binding、environment、modifier、preview 的长期参考。
- 参考来源：
  - https://developer.apple.com/xcode/swiftui/

### 定位

#### `positioning`

> SwiftUI 的生态定位是 Apple 平台级 native declarative UI framework，而不是单纯组件库或 headless
> primitive。它同时覆盖声明式视图描述、状态驱动更新、布局、动画、手势、accessibility representation、平台自适应控件、scene/window、preview，以及与 UIKit/AppKit 的互操作。

#### `target_users`

> 主要服务 iOS、iPadOS、macOS、watchOS、tvOS、visionOS 的原生应用开发者，尤其是希望用一套 Swift UI 语法在 Apple 生态内构建高一致性体验的产品团队、独立开发者、设计系统作者和平台级框架学习者。

#### `primary_value_proposition`

> 核心价值是用较少代码描述 UI 结果，由系统负责维护高效表示、状态依赖、平台外观、交互、动画和辅助功能输出。它与 open-gpui 的匹配点在声明式 native UI、state/data
> binding、modifier、environment、preview 和平台自适应理念；不匹配点在 Apple 专有 SDK、闭源运行时和强平台绑定。

### 分发与生态

#### `distribution_model`

> SwiftUI 随 Apple 平台 SDK 和 Xcode 分发，开发者通过 `import SwiftUI` 使用系统框架，并随 Xcode、iOS、macOS 等平台版本获得新 API。它不是 copy-to-
> own、registry、CLI add 或 npm 式包生态；应用自身可以通过 Swift Package Manager 分发 SwiftUI 组件库，但 SwiftUI 核心能力是平台 SDK 依赖。

#### `source_ownership`

> 开发者拥有自己写的 View、Modifier、Style、Layout、Observable model 和 UIKit/AppKit bridge 源码，但不拥有 SwiftUI 框架实现源码。升级和兼容成本主要来自
> Xcode/SDK/API availability、平台版本差异、行为变化和编译器诊断；框架 bug 通常只能绕过、提交反馈或等待 Apple 修复，无法像开源 crate 一样直接 patch。

### AI 时代设计

#### `ai_friendliness`

> 中高。SwiftUI 代码是强类型、声明式、层级清晰的 Swift 源码，`View`、`body`、property wrappers、modifiers、Preview、sample code 和 WWDC 文稿都利于 AI
> 检索、解释和局部改写。短板是 Apple 文档页面本身大量依赖 JavaScript，运行时闭源，组件行为 contract 并非以独立 schema 公开，AI 生成后的验证主要依赖编译、preview、测试和人工交互检查。

#### `copy_modify_verify_loop`

> SwiftUI 的常规闭环是编写或复制 View 代码，通过 Swift 编译器和类型系统快速发现 API 错误，用 Xcode Previews 在 canvas、设备、Dark Mode、横竖屏、不同文字大小、RTL
> 等上下文中验证，再用 XCTest/Swift Testing、UI tests 和真机或模拟器交互补齐回归验证。对 open-gpui，最值得借鉴的是“代码即组件源、preview 即即时反馈、环境矩阵即验证上下文”的闭环。

### API 与组合

#### `api_ergonomics`

> API 以 `View` 协议、`body: some View`、`@ViewBuilder` 声明式组合、链式 view modifiers、property wrappers 和强类型数据流为核心。常见调用形态是用
> `Text`、`Image`、`Button`、`List`、`HStack`、`VStack`、`NavigationStack` 等值类型组合 UI，再通过
> `.font()`、`.foregroundStyle()`、`.overlay()`、`.sheet()`、`.searchable()` 等 modifier 叠加行为和样式。调用体验紧凑、可组合、可局部抽取，适合原生应用开发。

#### `customization_model`

> 定制模型分为几层：用 modifiers 调整样式、布局、动画、手势和 presentation；用 `@State`、`@Binding`、`@Environment`、`@Observable`、`@Bindable`
> 管理数据流；用自定义 View、ViewModifier、Layout、Preference、EnvironmentKey 和 Style 协议抽象复用；用 UIView/NSView representable 或 controller
> representable 接入 UIKit/AppKit。它提供大量 escape hatch，但深层平台控件行为和渲染策略仍由 SwiftUI/系统控制。

#### `component_anatomy_model`

> SwiftUI 不采用 Radix/Ark 式 Root/Trigger/Content/Item/Indicator/Portal anatomy。复杂能力更多通过粗粒度语义控件、container、modifier、Style
> 协议、Environment 和 closure content 组合，例如 `Button` 有 action 与 label，`List`/`ForEach` 负责数据驱动行，`NavigationStack`
> 负责导航语义，`.sheet`/`.popover` 负责 presentation。对 open-gpui 的启发是语义控件和 modifier ergonomics；若要做 headless primitive，仍需要另行定义显式
> parts contract。

#### `state_ownership_model`

> SwiftUI 明确强调 single source of truth：`@State` 表示由当前 view 拥有的内部状态，`@Binding` 表示对子级提供双向引用，`@Observable`/`@Bindable`
> 让模型属性参与依赖追踪，`@Environment` 从上下文读取系统或应用级值。View 本身是值类型描述，不是长期存活的对象；状态由 SwiftUI 在幕后维护，数据变化后重新计算依赖 view 的 body 并更新输出。命令式
> runtime handle 不是主路径，更多用于 focus、dismiss、openWindow 等环境动作或平台桥接。

### Headless 与行为

#### `headless_boundary`

> SwiftUI 不是 headless 架构。行为逻辑、状态依赖、布局、渲染、平台视觉、accessibility representation、动画和 presentation 大多在同一个框架语义中协同工作；Style
> 协议、Modifier、自定义 Layout 和 Representable 提供一定分层，但不是 renderer-neutral behavior kernel。open-gpui 若要建立通用 UI 框架，应借鉴 SwiftUI 的
> ergonomics，同时把 headless 行为 contract、renderer adapter、theme recipe 和 AccessKit 映射拆得更清楚。

#### `accessibility_model`

> SwiftUI 会根据声明式 view hierarchy、语义控件、label、value、action 和环境自动生成辅助功能表示，并内置支持 Dynamic Type、Dark
> Mode、Localization、RTL、键盘导航等平台能力；开发者可通过 accessibility modifiers 补充 label、hint、value、traits、actions、sort priority、focus
> 等信息。对 open-gpui 来说，应把 AccessKit 节点、关系、焦点路径、动作和值变化作为核心输出，而不是把 accessibility 当作后置插件。

### 渲染与性能

#### `rendering_model`

> SwiftUI 是 native declarative retained UI 模型：View 是值类型描述，SwiftUI 在幕后维护高效 UI 表示，并用它生成屏幕内容、手势/交互输出和 accessibility
> representation。它不是 DOM/WebView，也不是传统 immediate mode；底层会桥接并利用各 Apple 平台的原生 UI、渲染、动画和输入系统。

#### `native_advantage`

> SwiftUI 展示的 native 优势是平台一致性、系统控件、动态字体、RTL/localization、Dark Mode、多窗口、键盘导航、widgets、watch/visionOS 适配、UIKit/AppKit 互操作和
> Xcode Previews。映射到 open-gpui，最应形成优势的场景是高密度桌面 UI、大文本/代码编辑、大列表/树/表格、低延迟输入、GPU 合成、窗口级 overlay、原生拖拽/菜单、跨平台 accessibility
> 和可交互 preview。

#### `web_ecosystem_advantage`

> Web/Tauri/Electron 生态仍强在浏览器标准、CSS 布局和动画、DOM/ARIA 成熟工具链、npm 组件数量、Storybook/Chromatic、Web 内容嵌入、跨平台分发和 AI 可检索样例规模。SwiftUI
> 的弱点也提示 open-gpui 不应早期追逐 Web 全量生态，而应在 native 桌面强项上建立差异，同时保留与 WebView、Markdown/HTML 渲染和外部设计 token 的互操作。

### 主题与设计系统

#### `theme_token_model`

> SwiftUI 没有公开为 DTCG 风格 token registry 设计；它更多依赖 Apple Human Interface Guidelines、系统颜色、材料、字体、SF Symbols、control
> styles、environment values、`tint`、`foregroundStyle`、color scheme、dynamic type、control size、locale 和平台默认样式。Theme
> 更像环境与样式协议的组合，而不是独立 token 文件。open-gpui 可借鉴 environment-driven style resolution，但需要自建可序列化 token schema。

#### `design_token_pipeline`

> SwiftUI 不是 DTCG、Style Dictionary 或 Tailwind-like transform 管线。设计资源通常来自 Asset Catalog、SF Symbols、系统颜色/字体/材料和平台 HIG，跨平台输出由
> Apple SDK 与运行时适配。对 open-gpui 来说，SwiftUI 只能作为“系统级语义 token 和环境适配”的参考；真正的 design token pipeline 应另建
> schema、转换、fallback、mode、state、variant 和 schema drift gate。

#### `style_customization_boundary`

> SwiftUI 默认由 framework 和平台系统负责原生控件视觉与交互一致性；应用通过 modifiers、Style 协议、Environment、自定义 View/Modifier/Layout 和 asset/theme
> 输入做局部或体系化定制；当需要完全自绘或特殊控件时使用 Canvas、自定义 Layout 或 UIKit/AppKit bridge。边界的优点是一致性强、代码少，缺点是某些系统控件内部结构和行为难以精细替换。

### 组件表面

#### `component_coverage`

> 覆盖面非常广，包含文本、图片、按钮、Toggle、Picker、Slider、Stepper、TextField、TextEditor、List、Table、Grid、Stack、ScrollView、Form、Section、NavigationStack、NavigationSplitView、TabView、Menu、ContextMenu、Sheet、Popover、Alert、Toolbar、Search、Focus、Gesture、Animation、Canvas、Timeline、Scene、WindowGroup、Document、Widgets
> 以及与 Swift Charts、SwiftData、MapKit、UIKit/AppKit 等框架的互操作。

#### `must_have_for_open_gpui`

> 对 open-gpui 必须借鉴的是声明式组合、轻量 View/Element 描述、modifier ergonomics、state/binding/environment、single source of truth、preview
> 矩阵、语义控件自适应、accessibility 作为输出之一、自定义 Layout/Style 扩展点，以及平台 bridge 的渐进采用能力。第一阶段不必追完整 SwiftUI 覆盖面，但应把这些基础机制做成稳定骨架。

#### `do_not_chase`

> 当前阶段不应追 Apple 平台专属 API、watchOS/visionOS/widget 细节、HIG 特定视觉规范、闭源 SDK 式集中发布、所有系统控件数量、Swift 语法糖逐字复刻、过度魔法化的隐式行为，以及
> UIKit/AppKit 互操作的具体形态。open-gpui 更应追 native Rust 桌面 UI 的清晰 contract、可测试行为、可组合样式和性能优势。

### 治理

#### `versioning_and_breakage`

> SwiftUI 的版本治理绑定 Apple 平台和 Xcode SDK，而不是 SemVer 包。新 API 通过 OS availability、编译器检查、文档和迁移资料暴露；老系统兼容需要 `@available`、条件分支或
> UIKit/AppKit fallback。对 open-gpui 来说，更适合采用 Cargo SemVer、feature gates、experimental API 标记、migration guide、schema version
> 和 examples 编译矩阵，同时借鉴 SwiftUI 的 availability 思维。

#### `maintenance_cost`

> 维护成本极高，属于平台厂商级工程：需要同时维护声明式 DSL、编译器/类型系统协作、布局、控件、动画、accessibility、预览、跨设备适配、UIKit/AppKit 互操作、文档、示例和多年 OS 兼容。open-gpui
> 不应试图一次性复刻 SwiftUI；更现实的是先把核心渲染、状态、modifier、environment、preview、accessibility 和少量高价值 primitive 做深。

#### `risks`

> 主要风险是被 SwiftUI 的平台级完整性误导，低估闭源 SDK、系统控件和 Xcode 集成背后的投入；过度追求隐式魔法会削弱 Rust API 的可解释性；行为、样式、布局和 accessibility 过度耦合会不利于
> headless 生态；缺少机器可读 contract 会让 AI 生成难验证；若照搬 Apple 专属语义，open-gpui 容易失去跨平台和桌面高性能差异化。

#### `open_gpui_relevance`

> 建议为 reference-only，并对 state/binding/environment、modifier、preview、semantic controls 和 accessibility-first rendering 做
> trial。不要采用 SwiftUI 的闭源 SDK 分发和 Apple 专属 API 面；直接设计含义是 open-gpui 应先形成“native declarative core + explicit behavior
> contracts + typed environment/theme + preview/gallery + AccessKit output + Cargo-native
> distribution”的架构，而不是先追一个庞大的全平台视觉组件库。

### 不确定字段（已跳过）

- `diagnostics_and_failure_quality`
- `docs_gallery_model`
- `interaction_state_machines`
- `machine_readable_contracts`
- `performance_model`
- `positioning_and_collision_model`
- `registry_viability`
- `rust_distribution_fit`
- `testing_strategy`
- `third_party_ecosystem_path`

## <a id="jetpack-compose"></a>13. Jetpack Compose

- 结果文件：`Jetpack_Compose.json`
- 调研类别：`native_declarative_ui`
- 纳入原因：
  Android/Kotlin 声明式 UI 与 state hoisting、modifier、material components 参考。
- 参考来源：
  - https://developer.android.com/compose

### 定位

#### `positioning`

> Jetpack Compose 的定位是 Android 官方推荐的现代原生声明式 UI framework，覆盖 runtime、compiler、UI/foundation、Material 组件、工具链、测试与文档。它不是
> headless primitive 库，也不是 copy-to-own 组件 registry，而是 Android/Kotlin 生态内的完整 UI 开发范式。

#### `target_users`

> 主要服务 Android/Kotlin 应用开发者、移动端产品团队、需要构建设计系统的 Android 团队、Android Studio/Gradle 工具链用户，以及希望用声明式 UI 替代 XML/View 体系的团队。

#### `primary_value_proposition`

> 核心价值是用 Kotlin composable 函数、状态驱动渲染、Modifier 链、Material 组件和 Android Studio 工具减少 UI 样板代码并提升迭代速度。对 open-gpui
> 的匹配点在声明式组合、state hoisting、modifier-like API、语义树测试和 lazy layout；不匹配点是 Android 生命周期、Gradle/Kotlin compiler 插件、移动端 Material
> 设计和平台服务绑定。

### 分发与生态

#### `distribution_model`

> Compose 采用 package dependency 分发：通过 Gradle、Google Maven、AndroidX artifacts 和 Compose BOM 管理
> runtime、ui、foundation、material/material3、animation、tooling、test 等库版本；Android Studio
> 模板、Preview、Codelab、示例仓库和官方文档构成外围入口。它不采用源码复制、CLI add、组件 marketplace 或 shadcn 式 registry 模式。

#### `source_ownership`

> 使用者默认不拥有组件源码，而是依赖 AndroidX 开源库的二进制 artifact；可以查看源码、fork AndroidX 或在本地 wrapper/design system 中封装，但常规路径是跟随 BOM 和 AndroidX
> 版本升级。升级成本主要来自 Compose BOM、Kotlin/AGP 兼容性、实验 API、Material 版本、行为变更和本地组件封装适配；相比 copy-to-own，行为修复集中但底层 patch 成本较高。

### API 与组合

#### `api_ergonomics`

> API 以 `@Composable` 函数、强类型参数、slot lambda、Modifier 链、state/value 回调和 Kotlin 默认参数为核心。调用体验像写普通函数：布局用
> Row/Column/Box/LazyColumn 等组合，外观和行为通过 Modifier 追加，复杂组件通过 content、icon、label、actions 等 slot 注入子结构，状态通过
> `value/onValueChange`、`expanded/onDismissRequest` 或上层 ViewModel 提升。

#### `customization_model`

> 定制模型分多层：轻量定制用 Modifier 改尺寸、padding、点击、语义、绘制和布局；组件参数和 slot 改结构与子组件；MaterialTheme 改 color scheme、typography、shapes
> 与动态颜色；Foundation 和自定义 layout/draw/semantics 提供 escape hatch；业务状态可放在 caller、state holder 或 ViewModel。它鼓励封装本地 design
> system composable，而不是直接改官方组件源码。

#### `component_anatomy_model`

> Compose 没有 Radix/Ark 那种统一 Root/Trigger/Content/Item/Indicator/Portal anatomy registry。复杂组件更多通过 composable 函数拆分、slot
> lambda、state 参数和 Modifier 组合表达，例如 Scaffold 的 topBar/bottomBar/snackbarHost/content，TextField 的
> label/leadingIcon/trailingIcon，DropdownMenu 的 expanded/onDismissRequest 与 DropdownMenuItem。对 open-gpui 的启发是 slot
> 组合很顺手，但若要服务 AI 和主题系统，应额外提供显式 anatomy/part metadata。

#### `state_ownership_model`

> Compose 的状态模型非常值得参考：局部 UI 状态用 `remember`/`mutableStateOf`，需要配置变更恢复时用 `rememberSaveable`，可复用组件推荐 state
> hoisting，把状态移动到调用方形成单一事实源；应用级状态通常进入 ViewModel 或 state holder。组件 API 常用 value/onValueChange、expanded/onDismissRequest
> 等受控形态，内部也可保留 transient state；状态变化触发 recomposition，UI 由当前状态重新描述。

### Headless 与行为

#### `headless_boundary`

> Compose 有清楚的 framework layering：runtime 管组合与状态，UI 层提供 Modifier、layout、draw、semantics，Foundation 提供基础行为和低层组件，Material
> 提供设计系统组件。但它不是严格 headless 架构，许多行为、语义、布局和渲染绑定在 composable/Modifier 中。open-gpui 可借鉴分层，但应把行为 contract、AccessKit 语义、overlay
> geometry、render adapter 和 style/theme 边界拆得比 Compose 更显式。

#### `accessibility_model`

> Compose 的可访问性核心是 Semantics：通过 contentDescription、role、stateDescription、mergeDescendants、clickable、custom
> actions、traversal order 等把 UI 意义暴露给 Android accessibility services，同时测试也使用语义树查找和断言节点。Material 组件通常内置基础语义；自定义组件需要显式补充
> label、状态、动作、焦点和阅读顺序。open-gpui 可将该思想映射到 AccessKit 的 role、name、value、action、relationship、focus order 和可测试语义树。

#### `interaction_state_machines`

> Compose 通常不把组件行为公开为有限状态机，而是用 state、callback、Modifier、InteractionSource、FocusRequester、gesture
> detector、coroutine/animation state 和测试 API 组合。它的状态提升原则能让行为可控，但 menu/select/dialog/tree 等复杂交互的状态转移不如 Zag/Ark
> 那样可视化和可独立测试。open-gpui 应借鉴 state hoisting，同时为复杂 desktop primitive 补上可测试的显式状态机或等价 contract。

### 渲染与性能

#### `rendering_model`

> Compose 是 Android 原生声明式增量渲染模型：composable 函数描述 UI，Compose runtime 跟踪状态读写并触发 recomposition，UI/layout/draw/semantics 节点最终落到
> Android 原生渲染与无障碍系统。它不是 DOM/WebView，也不是传统 XML View 树；对 open-gpui 更接近“声明式组合 + retained UI tree + 增量更新 + 原生绘制”的参考。

#### `performance_model`

> Compose 性能策略围绕减少无效工作：使用 `remember` 缓存昂贵计算，用 lazy layouts 展示长列表和网格，为 lazy item 提供稳定 key，用 `derivedStateOf` 限制
> recomposition，尽量推迟状态读取，让最小范围重组，避免 backwards writes，并通过 baseline profiles、benchmark、Layout Inspector 与 recomposition
> debugging 观察问题。它对大列表有 LazyColumn/LazyRow/LazyVerticalGrid/Paging 路径，但不是专门的大表格、大代码文本或桌面 docking 框架。

#### `web_ecosystem_advantage`

> Web/Tauri/Electron 生态在 CSS、DOM 可访问性、浏览器调试、Storybook、Chromatic、npm 包、设计 token 工具、跨平台运行和现成组件数量上更强。Compose 通过 Android
> 官方整合弥补了 Web 生态优势，但 open-gpui 没有同等规模平台背书，因此应避开早期追完整 Web 组件宇宙，优先做好 native 桌面高密度控件、性能、语义测试和与 Web/token 工具的互操作。

### 主题与设计系统

#### `theme_token_model`

> Compose Material 3 的主题由 MaterialTheme 承载，核心子系统是 ColorScheme、Typography 和 Shapes，并支持动态颜色、深浅色模式、tonal elevation 等 Material
> 概念；非 Material 设计系统也可用 CompositionLocal、Modifier、slot 和自定义 composable 建立主题。它的 token 更偏 Kotlin 类型和运行时对象，而不是独立 theme
> file/registry schema。

#### `style_customization_boundary`

> 样式边界由 framework 层级决定：runtime/UI/foundation 提供布局、绘制、输入和语义原语；Material 组件提供默认视觉、状态和主题消费；应用通过
> MaterialTheme、Modifier、组件参数、slot、本地 wrapper 和自定义 draw/layout 覆盖外观。对 open-gpui 来说，合理边界是核心组件只承诺结构、行为、语义和必要布局，主题 recipe
> 与应用源码负责最终视觉。

### 组件表面

#### `component_coverage`

> 覆盖度很高，官方 Compose/Material
> 生态包含文本、图片、Row/Column/Box、LazyColumn/LazyRow/Grid、Button、IconButton、Card、Checkbox、Switch、RadioButton、Slider、TextField、DatePicker、Dialog、DropdownMenu、Tooltip、Snackbar、ProgressIndicator、NavigationBar、NavigationRail、Drawer、TopAppBar、Scaffold、Pager、Canvas、animation、gesture、drag
> and drop、semantics、testing 和 adaptive layout 等。

#### `must_have_for_open_gpui`

> 对 open-gpui 必须吸收的能力是声明式 composable/builder 体验、modifier-like 可组合修饰器、state hoisting 与单一事实源、slot-based 组合、类型化 theme、语义树与测试
> matcher、lazy list/grid、Preview/gallery 式快速反馈、性能诊断和 Android Compose 那种“文档-示例-测试-工具链”一体化意识。第一阶段应优先落到
> Button/Input/Dialog/Menu/Popover/Tooltip/Tabs/List/Tree/Table/Text 这些桌面基础面。

#### `do_not_chase`

> 当前阶段不应追 Android 生命周期、Activity/ViewModel 绑定、Gradle/BOM 形态、Kotlin compiler 插件魔法、Material 3 全量移动端规范、Android Studio Preview
> 级别 IDE 集成、手机折叠屏/自适应移动端 API、Compose 与旧 View 系统互操作，以及所有 Android 平台特有组件。open-gpui 应追等价的架构原则，而不是追 Android 表面能力。

### 文档测试工具

#### `testing_strategy`

> Compose 有专门 UI testing APIs：通过 semantics 查找元素、验证属性、执行用户动作，并提供同步机制等待 UI idle；测试可分本地测试和 instrumented tests，也可结合
> accessibility、benchmark、Macrobenchmark、baseline profile、Layout Inspector 和 recomposition 调试。对 open-gpui
> 应升级为状态机单测、adapter contract、截图/视觉测试、交互回放、AccessKit 树断言、性能预算、API surface 与 schema drift gate。

### 治理

#### `versioning_and_breakage`

> Compose 通过 AndroidX release、Compose BOM、Gradle dependency、稳定/实验 API 标记、迁移文档和 Kotlin/AGP 兼容矩阵治理版本。BOM 用单一版本协调多 Compose
> library，降低依赖不一致；破坏性变化主要发生在实验 API、Material 版本迁移、compiler/Gradle 兼容和行为细节上。open-gpui 可借鉴 BOM 思路为 workspace crate 建立兼容矩阵和
> migration guide。

#### `maintenance_cost`

> 维护成本极高：Compose 同时维护 runtime、compiler、snapshot state、layout、draw、input、text、accessibility
> semantics、foundation、Material、animation、tooling、testing、Android interop、性能工具和大量文档示例。open-gpui 不应一开始复制这种全栈规模，而应先把
> runtime/Element、theme、semantics/testing、overlay、lazy layout 和少量高价值组件做深，再扩展组件面。

#### `risks`

> 主要风险是把 Android/Material/Kotlin 模型机械迁移到 Rust desktop，导致抽象错位；依赖 compiler 魔法会提高实现和调试成本；Modifier 过度自由可能让布局、语义和事件边界难以审计；slot
> API 若缺少 anatomy metadata 会降低 AI 可验证性；组件覆盖过快会稀释 native 性能优势；如果没有统一 contract，文档、示例、主题和测试容易漂移。

#### `open_gpui_relevance`

> 最终建议为 reference-only 偏 trial：不要采用 Compose 的 Android 平台实现，但应强参考其声明式 API、state hoisting、modifier、MaterialTheme
> 式主题、semantics/testing 和 lazy layout。直接设计含义是 open-gpui 需要显式的 Element/State/Modifier/Theme/Semantics/Test
> contract，并用少数组件试点验证“声明式组合 + 可测试语义树 + native 性能”是否能成为通用 UI 框架主线。

### 不确定字段（已跳过）

- `ai_friendliness`
- `copy_modify_verify_loop`
- `design_token_pipeline`
- `diagnostics_and_failure_quality`
- `docs_gallery_model`
- `machine_readable_contracts`
- `native_advantage`
- `positioning_and_collision_model`
- `registry_viability`
- `rust_distribution_fit`
- `third_party_ecosystem_path`

## <a id="flutter"></a>14. Flutter

- 结果文件：`Flutter.json`
- 调研类别：`cross_platform_rendered_ui`
- 纳入原因：
  跨平台自绘 UI、widget tree、theme、material/cupertino、hot reload 与生态分发参考。
- 参考来源：
  - https://flutter.dev/

### 定位

#### `positioning`

> Flutter 的生态定位是跨平台自绘 UI framework 和应用 SDK，而不是单纯组件库、headless primitive 或桌面 shell。它同时覆盖 Dart 语言运行时、声明式 widget
> tree、layout/render object 管线、Material/Cupertino 设计系统、platform embedder、DevTools、hot reload、测试工具、pub.dev 包生态和多端构建分发。

#### `target_users`

> 主要服务希望用一套 Dart 代码构建 iOS、Android、Web、Windows、macOS、Linux 应用的产品团队、移动/桌面应用开发者、设计系统作者、插件维护者和跨平台业务团队。对 open-gpui
> 来说，最相关的用户画像是需要原生性能、自绘一致性、可组合 UI 和工具链闭环的桌面产品团队。

#### `primary_value_proposition`

> 核心价值是用声明式 widget 描述 UI，由 Flutter 自己负责布局、绘制、合成、输入、语义和跨平台适配，从而减少系统控件差异并获得接近 native 的性能与一致体验。与 open-gpui 的匹配点是自绘 native
> UI、widget/element/render 分层、增量更新、主题体系、DevTools/inspector 和 hot reload 式反馈；不匹配点是 Dart/VM/SDK 级全栈平台投入过大，且 Flutter
> 面向移动和多端应用，open-gpui 更应聚焦 Rust 桌面 UI 与高密度产品场景。

### 分发与生态

#### `distribution_model`

> Flutter 核心通过 Flutter SDK、Dart SDK、`flutter` CLI、平台模板和官方文档分发；应用通过 `flutter create` 脚手架生成，依赖写入 `pubspec.yaml`，第三方库和插件主要通过
> pub.dev 与 `flutter pub add`/`dart pub add` 获取。包形态包括纯 Dart package、Flutter package、plugin package、FFI package、federated
> plugin、examples、tests、assets 和 platform-specific implementation。它不是 copy-to-own 组件 registry，而是 SDK + package dependency
> + scaffold + plugin ecosystem 的组合。

#### `source_ownership`

> 开发者拥有应用源码、widget 源码、主题、插件 glue code、平台 runner 和本地 package 源码；Flutter framework 与 engine 是开源的，可以阅读、fork、patch
> 或贡献，但常规使用路径仍是依赖 SDK 版本升级。升级成本集中在 Flutter/Dart SDK 版本、pub dependency resolution、breaking changes、平台构建链、插件平台实现和渲染行为变化；相比
> copy-to-own，统一 SDK 依赖能集中修复行为问题，但对单个应用的深度 patch 与长期 fork 成本较高。

### AI 时代设计

#### `ai_friendliness`

> 较高。Flutter 代码是强类型 Dart、声明式 widget tree、API reference 完整、docs 与 samples 丰富，DevTools inspector 能展示 widget tree 和属性；官方还提供
> `llms.txt`、AI rules、Dart/Flutter MCP server、package search、analysis、format、test 和运行时布局错误检查等 AI 辅助入口。短板是真实 UI 行为仍分散在
> widget 源码、render object、platform embedder 和插件实现中，AI 生成后必须依赖 analyzer、formatter、tests、inspector、截图和真机交互验证。

#### `copy_modify_verify_loop`

> Flutter 的闭环是复制或生成 widget 代码后，用 Dart analyzer 和类型系统检查 API，用 `dart format`/`flutter format` 统一风格，用 hot reload
> 在设备或模拟器上保留状态快速验证，用 Flutter inspector 查看 widget tree、layout、repaint 和 oversized image，用
> unit/widget/integration/golden/performance tests 补回归。对 open-gpui 最值得借鉴的是“源码即组件、CLI 一键运行、运行时 inspector、热反馈和测试同源”的闭环。

### API 与组合

#### `api_ergonomics`

> API 以声明式 widget 组合为核心：`StatelessWidget`/`StatefulWidget` 的 `build(BuildContext)` 返回 widget
> tree，layout、绘制、交互、动画、导航、主题都通过嵌套 widget、constructor props、callbacks、controllers、keys、InheritedWidget/Theme lookup 和
> builder/child closure 组合。调用体验成熟、可读、生态样例多，但深层自定义时需要理解 Widget、Element、RenderObject、Layer、Constraints、BuildContext 和生命周期边界。

#### `customization_model`

> 定制模型分层明显：外观通过 ThemeData、CupertinoThemeData、ColorScheme、TextTheme、component
> themes、ThemeExtension、MaterialState/WidgetState、IconTheme、DefaultTextStyle 和局部 widget props 调整；结构通过组合小
> widget、builder、slot-like child/children、custom painter、custom render object 和自定义 layout 调整；行为通过
> controllers、callbacks、Focus/Shortcuts/Actions、GestureDetector、ScrollController、Navigator/Route 和插件扩展。escape hatch
> 很多，但复杂度随自定义深度快速上升。

#### `component_anatomy_model`

> Flutter 不采用 Radix/Ark 式 Root/Trigger/Content/Item/Indicator/Portal 命名体系，而是用 widget composition 和
> child/children/builder/controller/route/overlay 拆分复杂组件。例如
> MaterialApp/Scaffold/AppBar/Drawer/BottomNavigationBar、Navigator/Route、Overlay/OverlayEntry、MenuAnchor/MenuItemButton/SubmenuButton、DropdownMenu、TabBar/TabBarView、ListView/ListTile
> 等形成事实上的 anatomy。对 open-gpui 来说，Flutter 证明“小 widget 组合”有效，但 headless primitive 仍应额外声明显式 parts contract，避免复杂组件 anatomy
> 只藏在源码约定里。

#### `state_ownership_model`

> Flutter 的基础状态模型是 widget immutable、element 持久、state 对象保存可变状态；局部状态用 `StatefulWidget` + `setState`，树级共享用
> InheritedWidget/InheritedModel/Theme/MediaQuery，应用级状态通常交给 provider、riverpod、bloc、redux、mobx 等包；输入、滚动、动画、焦点和文本编辑常用
> Controller/FocusNode/AnimationController 等 runtime handles。它支持 application-owned state、component-owned state 和 lifted
> state，但 controlled/uncontrolled 不是统一命名规范，更多体现在 value/controller/callback/default 参数约定中。

### Headless 与行为

#### `headless_boundary`

> Flutter 不是严格 headless 架构。行为、状态、layout、绘制、semantics、theme 和 platform adaptation 经常在同一个 widget 或 render object 体系中协同工作；底层有
> gestures、rendering、semantics、painting、widgets、material、cupertino 等层，但高层组件通常同时包含视觉和行为。open-gpui
> 应借鉴其分层工程能力和组合体验，同时把行为状态机、AccessKit 语义、overlay 几何、theme recipe 与 GPUI render adapter 拆得更显式。

#### `accessibility_model`

> Flutter 在 framework 与 engine 层提供 semantics tree，并由 platform embedder 接入底层系统 accessibility、input 和 rendering
> surfaces；开发者可使用 Semantics、MergeSemantics、ExcludeSemantics、Focus、Shortcuts、Actions、Tooltip、label/value/hint
> 等机制补充语义。Material/Cupertino 内置控件通常自带较多语义，官方文档强调 WCAG、EN 301 549、VPAT、屏幕阅读器、动态字体、国际化和可访问性检查清单。对 open-gpui 来说，应把 AccessKit
> node、role、name、value、action、relationship、focus order 和语义快照测试作为一等输出。

### 渲染与性能

#### `rendering_model`

> Flutter 是自绘 native retained/declarative UI 模型：Dart widget tree 生成 element tree，element 连接 render object tree，render
> object 负责 layout、paint、hit testing 和 semantics，framework 合成 scene，engine 用 Impeller/Skia 相关后端 rasterize 到平台
> surface。它不依赖系统原生控件，也不是 DOM/WebView；Web 端使用 CanvasKit/Skwasm 等 renderer。

#### `native_advantage`

> Flutter 展示的 native 自绘优势是整屏合成、跨平台视觉一致、避免频繁跨平台控件桥接、AOT 启动和执行性能、GPU 渲染、可控 layout/paint pipeline、统一手势和动画系统、可嵌入平台能力以及
> DevTools 可观测性。映射到 open-gpui，最应形成优势的是高密度桌面 UI、代码/富文本编辑、大表格/树/列表、低延迟输入、复杂浮层、命令面板、窗口/多显示器/DPI 坐标、GPU 合成和 AccessKit 深集成。

#### `web_ecosystem_advantage`

> Web/Tauri/Electron 仍天然强在 DOM/CSS/ARIA 标准、浏览器渲染兼容、WebView 内容嵌入、npm 组件规模、Storybook/Chromatic、CSS 设计 token 工具、Web dev
> hiring、调试生态和 SaaS 管理后台组件覆盖。Flutter Web 本身也提示自绘 UI 在文本选择、SEO、浏览器原生语义、HTML 混排和 DOM 互操作上有代价。open-gpui 不应追 Web 全量生态，应保留
> WebView/HTML/Markdown/设计 token 互操作，并在 native 桌面强项上差异化。

### 主题与设计系统

#### `theme_token_model`

> Flutter 的主题模型以 inherited context 和 typed theme data 为核心：MaterialApp 挂载 ThemeData，Theme.of(context) 查找最近 Theme；ThemeData
> 包含 ColorScheme、TextTheme、component theme、visual density、brightness、state layer、extensions
> 等；CupertinoApp/CupertinoThemeData 提供 iOS 风格主题；局部 Theme 可以覆盖子树。它更像运行时 typed theme object + inherited lookup，而不是独立 DTCG
> token 文件。

#### `style_customization_boundary`

> Flutter 的样式边界分为四层：framework 提供 Material/Cupertino 默认视觉和行为；ThemeData/CupertinoThemeData/component theme 提供应用级和子树级默认值；具体
> widget props 负责局部覆盖；用户源码、CustomPainter、RenderObject、ThemeExtension 和第三方 package 提供深度 escape
> hatch。这个边界让普通应用很快成型，但在设计系统严格治理时容易出现 theme、props、custom widget、package 多处同时定义样式的漂移。

### 组件表面

#### `component_coverage`

> 覆盖面极广：基础控件、文本、图片、图标、按钮、输入、表单、选择器、滑块、开关、checkbox/radio、layout、scroll、sliver、list/grid/table、navigation、route、tabs、drawer、app
> bar、menu、tooltip、dialog、bottom sheet、snack bar、progress、animation、gesture、focus、shortcut/action、semantics、canvas/custom
> paint、Material、Cupertino、adaptive/responsive、platform views、plugin integration、desktop/web 支持以及大量 pub.dev 第三方组件。

#### `must_have_for_open_gpui`

> 必须借鉴的是 widget/element/render 分层、不可变 UI 描述 + 持久运行时对象、constraints layout、sliver/virtual scrolling 思路、typed theme +
> inherited context、inspector/diagnostics、hot reload 或接近热反馈的开发闭环、官方组件目录、pub.dev 式质量信号、插件分层和 accessibility-first 输出。open-
> gpui 第一阶段不必追 Flutter 的全平台 SDK，而应先补齐
> Button、TextField、Checkbox、Radio、Switch、Menu、Popover、Dialog、Tooltip、List/Table/Tree、Tabs、Form
> Field、Focus、Shortcut、Overlay、Theme 和 Gallery/Test contract。

#### `do_not_chase`

> 当前阶段不应追 Dart VM/JIT/AOT 工具链、移动端全平台模板、Flutter Web renderer、Material/Cupertino 全量控件、移动传感器/平台插件生态、App Store/Play Store
> 发布链、复杂 hot reload 运行时、完整 DevTools 套件、动画/导航/路由所有高级能力，以及跨端像素级一致的庞大 SDK。open-gpui 更应聚焦 Rust 桌面 UI、GPUI 原生能力、明确
> contract、可测试组件和高性能应用场景。

### 治理

#### `versioning_and_breakage`

> Flutter 版本治理绑定 Flutter SDK、Dart SDK、pub 包版本、release notes、breaking changes、migration guide 和平台 toolchain；应用通常通过 `flutter
> upgrade`、pub dependency constraints 和 lockfile 控制升级。第三方包遵循 pub 版本约束和语义化版本惯例，插件还要维护平台实现兼容。对 open-gpui 来说，应采用 Cargo
> SemVer、feature gates、experimental 标记、schema version、migration guide、examples 编译矩阵和 breaking-change lint，避免 SDK
> 级集中升级带来的大面积行为漂移。

#### `maintenance_cost`

> 维护成本极高，属于平台级 SDK 工程：需要维护 Dart/Flutter framework、engine、Impeller/Skia
> 后端、text/layout/semantics、Material/Cupertino、platform embedders、CLI、templates、DevTools、packages、插件接口、Web/Desktop/Mobile
> 构建链、文档、示例、测试和社区治理。open-gpui 不应一次性复刻 Flutter，而应把少数核心机制做深：Element/Entity 分层、layout/render
> pipeline、theme、overlay、a11y、inspector、gallery 和高价值组件。

#### `risks`

> 主要风险是被 Flutter 的完整 SDK 成功路径误导，低估统一语言、VM、engine、CLI、DevTools 和平台团队投入；第二是过度自绘导致与系统控件、文本、输入法、accessibility、Web
> 内容互操作成本升高；第三是 widget 组合过细会带来深树、诊断和学习成本；第四是 theme/props/custom widget 多层样式边界可能漂移；第五是如果没有机器可读 contract，AI
> 生成组件会出现视觉可用但语义、焦点、性能不可验证的问题。

#### `open_gpui_relevance`

> 建议 reference-only + targeted trial。不要采用 Flutter 的 Dart SDK、移动优先平台层和全量组件路线；应重点试验 widget/element/render object
> 分层、constraints layout、typed inherited theme、inspector diagnostics、hot feedback、pub.dev 式生态质量信号、plugin 分层和 semantics-
> first 渲染。直接设计含义是 open-gpui 应形成“Cargo-native distribution + typed component contract + GPUI retained/native render +
> AccessKit output + theme/token schema + gallery/test/AI docs 同源”的框架，而不是只做一套视觉组件。

### 不确定字段（已跳过）

- `design_token_pipeline`
- `diagnostics_and_failure_quality`
- `docs_gallery_model`
- `interaction_state_machines`
- `machine_readable_contracts`
- `performance_model`
- `positioning_and_collision_model`
- `registry_viability`
- `rust_distribution_fit`
- `testing_strategy`
- `third_party_ecosystem_path`

## <a id="slint"></a>15. Slint

- 结果文件：`Slint.json`
- 调研类别：`rust_native_ui_framework`
- 纳入原因：
  > Rust/C++/JS declarative UI toolkit，适合比较 DSL、native deployment、component composition 和 embedded/desktop 场景。
- 参考来源：
  - https://slint.dev/

### 定位

#### `positioning`

> Slint 的生态定位是跨 Rust、C++、JavaScript、Python 的声明式原生 UI framework 与设计工具链，不是单纯组件库或 headless primitive。它以 `.slint` DSL
> 描述组件树、属性绑定、状态、动画和资源，再由编译器生成宿主语言接口，面向桌面、嵌入式、移动和 WebAssembly 场景。对 open-gpui 来说，Slint 最值得参考的是“声明式 UI 合约 + 强编译诊断 + 多后端渲染 +
> 设计/开发工具链”的系统工程，而不是照搬独立 DSL。

#### `target_users`

> 主要服务嵌入式设备团队、跨平台桌面应用开发者、需要 Rust/C++/JS 互操作的产品团队、UI 设计师与工程协作团队，以及希望在资源受限设备上获得现代 UI 的厂商。对 open-gpui 更相关的受众是 Rust
> 原生桌面产品团队、框架作者和组件生态维护者；嵌入式和 C++ 用户是重要参考，但不是 open-gpui 当前最直接的核心用户。

#### `primary_value_proposition`

> 核心价值是用一套专用声明式语言把 UI 结构、状态绑定、动画、皮肤和资源从业务逻辑中分离，并通过编译器、语言服务器、预览器、Figma 导入、测试 API 和多渲染后端形成完整闭环。它与 open-gpui 的匹配点在于静态可分析
> UI、跨平台原生渲染、组件封装、工具链诊断和可测试性；不匹配点是 Slint 选择 DSL-first、多语言绑定和嵌入式商业授权，而 open-gpui 更应优先保持 Rust/GPUI 原生 API、Cargo 生态和桌面产品性能优势。

### 分发与生态

#### `distribution_model`

> Slint 采用混合分发：Rust 侧通过 `slint`、`slint-build` 等 Cargo crate 集成，C++ 侧通过 CMake 包和生成代码集成，JavaScript/Python 侧通过对应语言包，UI 文件以
> `.slint` 源码随应用分发并在构建期编译，也支持解释器式动态加载。内置标准控件和样式随框架分发，Material 组件以独立源码/模板形式提供，开发工具包括 VS Code 扩展、语言服务器、slint-
> viewer、SlintPad、Figma 插件和测试 API。它不是 shadcn 式统一源码 registry，也不是插件市场；更像“核心 runtime + 编译器 + 语言绑定 + 工具链 + 示例/模板”的平台型分发。

#### `source_ownership`

> 应用开发者拥有自己的 `.slint` UI 源码、业务层 Rust/C++/JS/Python 代码、资源文件和自定义组件源码；标准组件、运行时和渲染器默认作为上游依赖使用。用户可以 fork Slint 或复制 Material
> 组件源码进行定制，但框架级修改会带来编译器、运行时、后端和授权治理成本。升级成本主要来自 DSL 语义、生成接口、控件行为、样式和渲染后端变化；相比 copy-to-own 组件库，Slint 的行为修复更集中，但深度改框架内部的成本更高。

#### `rust_distribution_fit`

> Slint 与 Rust 分发高度适配：通过 Cargo 依赖、build script、过程宏/编译期代码生成、feature flags 和 backend feature 组合接入，应用可以把 `.slint` 文件纳入
> workspace 构建，并把生成的强类型接口暴露给 Rust 业务层。缺点是 DSL 与 Rust 类型系统之间存在生成边界，复杂逻辑需要在 Slint 语言和 Rust 之间分工，IDE、格式化、lint、测试和 SemVer
> 也要同时覆盖 DSL 与 Rust API。open-gpui 若参考 Slint，应优先让组件 contract 与 Rust 源码同仓、同测、同版本，而不是引入会割裂 Cargo 工作流的第二语言。

### AI 时代设计

#### `ai_friendliness`

> 中高。Slint 的 `.slint` 语言是声明式、静态可解析、组件边界清晰，配套语言服务器、实时预览、编译错误、Rust API 文档、示例和 AI Coding Assistants 文档，适合 AI
> 检索、生成和用编译器验证基本正确性。限制是 DSL 生态语料远少于 HTML/React/Rust，复杂行为跨 `.slint` 与宿主语言时需要模型理解生成接口和绑定语义。对 open-gpui 的启示是：即使不采用
> DSL，也应提供机器可读 component spec、示例、错误诊断和快速验证命令，让 AI 生成后能被编译器、交互测试和截图测试约束。

#### `copy_modify_verify_loop`

> Slint 的循环是创建或复制 `.slint` 组件，使用实时预览和 VS Code/LSP 诊断修改 UI，再通过编译器生成宿主接口，用 Rust/C++/JS/Python 代码驱动数据和回调，最后通过测试
> API、截图/交互验证和真实目标设备运行确认。Material 组件源码和示例可以进入 copy-modify 路径，但框架标准控件更多依赖上游包。open-gpui 可借鉴这个闭环：组件 recipe 生成后必须能本地改源码，并通过
> `cargo check`、组件 contract test、visual snapshot、a11y metadata test 和性能 case 验证，而不是只靠示例能跑。

### API 与组合

#### `api_ergonomics`

> API 形态是 `.slint` 声明式组件 + 宿主语言强类型绑定。UI 侧通过属性、双向绑定、回调、函数、状态、动画、repeat/model、组件组合和样式继承表达界面；Rust
> 侧通过生成的组件句柄、getter/setter、callback 注册、model 类型和事件循环接入业务。优点是 UI 代码简洁、设计师/工程师边界清楚、工具能静态检查；代价是宿主语言开发者需要学习
> DSL，复杂抽象可能被分散到生成代码、全局单例、model 和回调之间。open-gpui 可参考其声明式绑定体验，但应优先提供 Rust-native builder/element/component API
> 与可选宏，而不是强制所有用户跨语言。

#### `customization_model`

> Slint 的定制主要发生在四层：自定义 `.slint` 组件结构，选择或覆盖内置 widget style，使用 palette、style metrics、状态和属性绑定控制视觉，必要时在宿主语言中提供
> model、回调和自定义逻辑。高级用户可以复制 Material 组件或自建控件库，嵌入式团队还可选择软件/GPU/Qt 等后端和平台适配。它的 escape hatch 比典型 Web headless
> 组件更偏框架级：一旦要改标准控件内部行为，往往需要复制组件源码、替换样式或深入框架。open-gpui 应把行为 primitive、theme recipe、component wrapper 和 app adapter 分层得更显式。

#### `component_anatomy_model`

> Slint 有组件组合模型，但不是 Radix/Base UI 那种公开的 `Root/Trigger/Content/Item/Indicator/Portal` anatomy。复杂控件通常封装为 `.slint` 组件，内部由
> Rectangle、Text、Image、TouchArea、FocusScope、ListView、PopupWindow 等元素组合；对外暴露属性和回调，而不是每个 part 都作为 public child 由用户拼装。对 open-
> gpui 的启示是两面的：Slint 证明封装式组件让普通开发者更快交付；但 open-gpui 若要建设通用组件生态，仍需要为 Dialog、Popover、Menu、Select、Tabs 等定义可复用 anatomy 和
> slot/part contract，否则深度定制会退化成复制整组件。

#### `state_ownership_model`

> Slint 的状态模型以声明式属性绑定、in/out/in-out 属性、回调、全局对象、model 和 states/transitions 为核心。简单 UI 状态可以留在 `.slint`
> 内部，业务状态通常由宿主语言拥有并通过属性、model 或回调同步；列表等数据由 ModelRc、VecModel 等模型抽象桥接。它不以 React 式 controlled/uncontrolled 术语组织
> API，但等价边界存在：UI 局部视觉状态适合组件内持有，业务真源适合应用持有。open-gpui 应显式定义哪些状态属于 behavior primitive，哪些属于 app entity，哪些只能通过 runtime handle
> 处理测量、焦点和窗口操作。

### Headless 与行为

#### `headless_boundary`

> Slint 不是 headless-first 框架，行为、布局、渲染、样式、动画和可访问性属性都在同一 DSL/运行时体系内协作。它有清晰的“UI 声明 vs 业务逻辑”边界，但没有把行为状态机、a11y
> metadata、layout/positioning、render adapter、style/theme 拆成可独立替换的 headless primitives。对 open-gpui 来说，Slint 可参考编译诊断和声明式绑定，但
> headless 边界应更细：行为核心不依赖视觉样式，AccessKit contract 不依赖主题，overlay geometry 不依赖具体组件树，theme recipe 只消费状态和 tokens。

#### `accessibility_model`

> Slint 提供可访问性属性和语义入口，例如 accessible role、label、description、value、action 等概念，并在标准控件、焦点处理和键盘交互中承载一部分 a11y
> 行为。官方最佳实践强调语义文本、键盘导航、焦点和可访问测试，工具链也在测试 API 和平台后端中暴露相应能力。与 Web ARIA 相比，Slint 的优势是可在 native/embedded
> 场景统一抽象；风险是不同后端、平台和标准控件对屏幕阅读器的覆盖质量需要真实设备验证。open-gpui 应把 a11y 作为一等 contract，明确映射到 AccessKit 的
> role、label、value、action、relationship、focus order 和测试断言。

#### `interaction_state_machines`

> Slint 语言提供 `states`、`when` 条件、transitions、animations、TouchArea、FocusScope、键盘事件和回调机制，可描述视觉状态和交互反馈；标准控件内部也包含自己的状态管理。它没有把
> Menu、Select、Combobox、Dialog 等复杂控件的有限状态机作为公开 contract 暴露。open-gpui 应学习其声明式状态/动画表达，但对复杂行为 primitive
> 应更进一步：公开可测试的状态图、事件表、键盘表、dismiss/focus 策略和诊断输出，避免行为隐藏在控件实现内部。

### 渲染与性能

#### `rendering_model`

> Slint 是原生渲染框架而非 WebView/DOM。UI 由 `.slint` 编译为运行时对象树，经过布局和属性绑定更新后交给不同后端渲染；常见后端包括
> winit/软件渲染、FemtoVG/OpenGL、Skia、Qt，以及面向嵌入式平台的 MCU/裸机或平台适配。它更接近 retained/declarative UI + 自绘/GPU/软件后端的组合，而不是 immediate
> mode。WebAssembly 目标存在，但依然以 Slint runtime 和 canvas/WebGL 等方式落地，不是复用浏览器 DOM 组件。

#### `performance_model`

> Slint 的性能策略围绕编译期 UI 分析、属性绑定依赖追踪、增量更新、模型驱动列表、资源压缩、可选软件/GPU 后端、嵌入式友好内存占用和避免 WebView 开销展开。大列表依赖 model 与
> ListView/virtualization 类能力，动画和绘制由 runtime/backends 管理。对 open-gpui 的启示是：通用 UI 框架必须把大列表、文本、canvas、overlay、滚动所有权、增量 diff 和
> GPU scene 放进基础架构，而不是让每个组件自行优化。Slint 的嵌入式优先约束也提醒 open-gpui 不要让主题、组件 registry 和示例系统把核心 runtime 变重。

#### `native_advantage`

> Slint 展示了 native UI 相比 WebView/DOM 的优势场景：低资源嵌入式设备、启动体积可控、GPU/软件后端可选、宿主语言直接调用、平台窗口与事件集成、无浏览器运行时依赖、硬件和显示目标可定制。open-gpui
> 应把优势聚焦到桌面生产力应用：大文本/代码编辑、大表格/树、命令面板、低延迟输入、多窗口/多显示器、高 DPI、GPU 合成、原生拖放、AccessKit 语义和可诊断 overlay。Slint 的“跨嵌入式到桌面”范围很宽，open-
> gpui 不必一开始追那么广。

#### `web_ecosystem_advantage`

> Web/Tauri/Electron 生态仍然在现成组件数量、CSS 设计系统、浏览器 a11y 实战、地图/图表/富文本、支付/登录/第三方 SDK、招聘和文档语料上更强。Slint 通过 DSL
> 和工具链降低了跨平台原生门槛，但也无法直接继承 Web 组件生态。open-gpui 应承认这些优势，避免早期追完整 Web 组件面；更合理的是提供 Web 心智可迁移的术语、组件 anatomy、主题
> token、示例对照和必要互操作，同时在 native 高价值场景形成明显优势。

### 主题与设计系统

#### `theme_token_model`

> Slint 的主题模型包括内置 widget style、Palette、StyleMetrics、颜色/字体/尺寸属性、状态绑定、样式继承和平台/后端相关默认外观。它不像现代设计系统那样公开完整 DTCG token schema，也不像
> Tailwind 那样把 utility token 作为主要 API。open-gpui 可借鉴 Palette/StyleMetrics 的轻量全局主题入口，但应设计更明确的 semantic token、component
> token、state token、mode、fallback、runtime theme 和 schema drift gate，保证组件、gallery、AI 生成和视觉回归测试共享同一事实源。

#### `style_customization_boundary`

> Slint 的样式边界偏框架内聚：标准控件样式由框架和选定 widget style 提供，用户通过属性、palette、状态、组件封装和复制源码调整视觉；业务应用通常不直接拆解标准控件内部 anatomy。open-gpui
> 若要支持更强生态，应采用更分层的边界：core framework 负责行为、布局、渲染和 a11y；theme recipe 负责默认视觉；component prop 负责受控变体；用户源码可替换 parts；app adapter
> 负责平台策略。这样既保留 Slint 式易用性，也避免深度定制只能 fork 控件。

### 组件表面

#### `component_coverage`

> Slint 覆盖基础 UI 元素、布局、文本、图片、输入、按钮、复选框、单选、滑块、spin box、combo box、tab、group box、scroll view、list view、standard
> list/table/tree 类视图、popup/window/dialog/menu/tooltip 相关能力，以及 Material 组件、标准控件样式和示例。覆盖面偏“可构建完整应用 UI 的 framework”，不是专门面向
> headless overlay、复杂数据表格或应用壳的组件生态。对 open-gpui 而言，Slint 的基础控件覆盖可作最低基线，但 table/tree/text/editor/docking 等桌面生产力组件仍需按 GPUI
> 优势单独设计。

#### `must_have_for_open_gpui`

> 必须借鉴的是静态可分析组件 contract、强错误诊断、实时预览/示例驱动、属性绑定语义、model-driven list、状态/动画表达、可访问性属性、跨后端抽象纪律和构建期验证。open-gpui 的第一阶段不一定需要 Slint
> 式 DSL，但必须拥有可组合的 Rust-native UI contract、统一 theme tokens、基础控件、overlay kernel、List/Table/Tree
> 的数据模型、docs/gallery、visual/interaction/a11y/performance gates，以及 AI 能调用的最短验证路径。

#### `do_not_chase`

> 当前阶段不应追 Slint 的多语言全覆盖、嵌入式 MCU 全平台、C++/JS/Python 绑定、独立 DSL-first 生态、完整设计工具商业链路、所有 widget style 和商业授权模型。open-gpui 更应集中在
> Rust 桌面原生 UI、GPUI Element/Entity 模型、AccessKit、GPU/文本/表格/overlay 性能、Cargo 分发和可验证组件生态。照搬 Slint DSL 会增加学习成本，并可能削弱 Rust 用户对
> API、类型和重构工具的直接掌控。

### 治理

#### `versioning_and_breakage`

> Slint 采用包版本、文档版本、release notes 和语言/API 演进管理破坏性变化。由于它包含 DSL、生成代码、运行时、后端、标准控件和多语言绑定，breaking change 的影响面比普通 Rust
> 组件库更大；需要同时考虑 `.slint` 源兼容、生成宿主接口兼容、样式/控件行为兼容和后端 feature 兼容。open-gpui 应保持更窄的稳定面：核心 behavior/render/a11y contract
> 保守演进，theme recipe 和 gallery 可快迭代，registry metadata 必须带兼容范围和迁移指南。

#### `maintenance_cost`

> 维护成本很高。Slint 需要长期维护 DSL 编译器、语言服务器、渲染后端、平台适配、多语言绑定、标准控件、样式、设计工具、测试 API、文档和商业/开源授权。open-gpui
> 不应复制这个全栈负担；应把资源集中在少数能形成生态杠杆的基础设施：Rust-native component contract、overlay kernel、a11y bridge、theme
> token/schema、gallery/test harness、性能基准和高价值桌面控件。等核心闭环稳定后，再扩展工具链和可视化设计协作。

#### `risks`

> 主要风险是把 Slint 的 DSL-first 路线误判为 open-gpui 的必要条件，导致 Rust 用户需要学习第二语言并承受生成边界；第二个风险是追逐嵌入式、桌面、移动、Web、多语言同时覆盖，稀释 open-gpui
> 在桌面原生性能上的重点；第三个风险是标准控件封装过深，缺少 public anatomy 和 headless contract，第三方组件难以安全定制；第四个风险是没有统一 schema 时，docs、gallery、AI
> 示例、测试和主题容易漂移；第五个风险是商业授权/双授权模式不适合 open-gpui 的社区预期。

#### `open_gpui_relevance`

> 建议为 trial + reference-only：Slint 的工具链、静态 UI 合约、属性绑定、model、状态动画、预览、测试和多后端抽象值得试点参考；DSL-first、多语言绑定、嵌入式全覆盖和商业授权路线只作
> reference-only，不应直接采用。直接设计含义是 open-gpui 应先定义 Rust-native、machine-readable 的 component contract，再让
> docs、gallery、scaffold、theme、a11y、visual tests 和 AI examples 从同一事实源派生；同时保持行为 primitive 和渲染元素分层，避免封装式 widget 牺牲可定制性。

### 不确定字段（已跳过）

- `design_token_pipeline`
- `diagnostics_and_failure_quality`
- `docs_gallery_model`
- `machine_readable_contracts`
- `positioning_and_collision_model`
- `registry_viability`
- `testing_strategy`
- `third_party_ecosystem_path`

## <a id="iced"></a>16. Iced

- 结果文件：`Iced.json`
- 调研类别：`rust_native_ui_framework`
- 纳入原因：
  Rust Elm-style GUI，适合比较 state/update/view 架构、widget ecosystem 和 renderer abstraction。
- 参考来源：
  - https://iced.rs/

### 定位

#### `positioning`

> Iced 是 Rust 原生跨平台 GUI framework，同时包含 widget 库、renderer/runtime 抽象、windowing shell、示例和测试工具。它不是 headless primitive 库，也不是
> shadcn 式源码 registry；更接近“Elm-style 应用框架 + batteries-included widgets + 可替换渲染后端”。

#### `target_users`

> 主要服务 Rust 应用开发者、桌面/跨平台工具团队、希望用 Rust 写 WebAssembly canvas UI 的开发者，以及想研究 Elm architecture、native widget runtime、renderer
> abstraction 的框架作者。

#### `primary_value_proposition`

> 核心价值是用简单、类型安全、响应式的 State/Message/update/view 模型构建跨平台 UI，并在 Cargo 生态内提供内置控件、异步任务、订阅、调试、测试、GPU/软件渲染后端和自定义 widget 扩展点。它与
> open-gpui 的匹配点在于 Rust-native API、renderer-agnostic runtime、widget tree 生命周期、debug/test tooling 和 Cargo 分发；不匹配点是 Iced
> 是完整应用框架，不是 GPUI 上层的 headless behavior/component contract。

### 分发与生态

#### `distribution_model`

> 分发以 Cargo package dependency 为主，核心 `iced` crate 聚合
> `iced_core`、`iced_runtime`、`iced_widget`、`iced_renderer`、`iced_winit`、`iced_wgpu`、`iced_tiny_skia` 等 workspace
> crate；功能通过 feature flags 打开或裁剪，如 `wgpu`、`tiny-skia`、`image`、`svg`、`canvas`、`markdown`、`lazy`、`debug`、`time-
> travel`、`hot`、`tester`、`tokio`、`smol`、`x11`、`wayland`。示例以仓库内 examples crate 分发；没有官方组件 registry、CLI add 或 copy-to-own 模式。

#### `source_ownership`

> 用户通常不拥有框架源码，只通过 crates.io/git dependency 消费 crate；可 fork 或 patch dependency，但升级成本由 Iced 的公开 API、feature flags 和 SemVer
> 决定。应用层 view/update/widget 代码完全归用户所有；内置 widget 行为和渲染细节是依赖包黑盒。对 open-gpui 来说，Iced 更像核心框架依赖模型，不适合直接证明源码复制型组件生态可行。

#### `rust_distribution_fit`

> 适配度很高。Iced 的 crate 拆分、feature flags、SemVer、examples-as-crates、可选 executor、可选 renderer 和 docs.rs 文档都符合 Rust/Cargo 生态。对
> open-gpui 的启发是：核心 crate 应清楚拆出 core/runtime/widget/renderer/window/test/devtools，避免一个巨型 crate 承担所有边界；同时要控制 feature
> graph、编译时间、公开类型泄漏和 breaking change 面。

### AI 时代设计

#### `copy_modify_verify_loop`

> Iced 的常规闭环是添加 Cargo dependency、实现 state/message/update/view、组合内置 widget 或实现自定义 `Widget`，再用 `cargo
> check/test/run`、examples、`iced_test` headless simulator、snapshot、E2E recorder、debug/time-travel/devtools
> 和人工运行验证。它不是复制内置组件源码后本地改造的模型。open-gpui 可借鉴其“Rust 编译 + headless interaction + screenshot + recorder”闭环，但应补组件 scaffold 后的
> contract gate、AccessKit 树断言、theme token drift 检查和 AI 可读失败输出。

### API 与组合

#### `api_ergonomics`

> API 以 Elm architecture 为核心：应用拥有 State，用户事件产生 Message，`update` 处理消息并返回 Task，`view` 返回 `Element`；`subscription`
> 监听外部事件；`theme/style/scale_factor/title` 等由 Program 提供。Widget 层使用函数、宏和 builder 方法组合，如
> `column![]`、`row![]`、`button(...).on_press(...)`、`text_input(...).on_input(...)`。高级自定义 widget 需要实现
> `size/layout/draw/update/operate/overlay/diff` 等方法，能力强但门槛高。

#### `customization_model`

> 样式通过 `Theme`、widget style/catalog、closure 或自定义类型扩展；结构通过组合 widget tree、container/row/column/stack/float/pin/responsive
> 等布局 widget 调整；行为通过 Message/update、widget state、Task/Subscription 和自定义 Widget 实现；渲染可选择 wgpu/tiny-skia 或底层 renderer
> 抽象。它提供较强 escape hatch，但不是 CSS token 或 slot recipe 模型，复杂组件的局部替换能力取决于具体 widget API。

#### `state_ownership_model`

> 应用状态由用户结构体拥有，Iced runtime 按 Message 调用 update；异步和副作用用 Task，外部事件用 Subscription。多数现代 widget 不要求用户维护 button/text_input
> 这类低层内部 state，内部状态存在 widget tree 中并通过 diff/tag/state 维持连续性；复杂 widget 如 combo_box、pane_grid、text_editor 等仍会有显式 State 或
> Content 由应用持有。整体是 application-owned state + framework-retained widget state 的混合模型，而不是 Web controlled/uncontrolled props
> 命名体系。

### Headless 与行为

#### `headless_boundary`

> 边界部分清楚：`iced_core`/`iced_runtime`/`iced_widget`/`iced_renderer`/`iced_winit`/renderer crates 拆开了核心类型、运行时、控件、渲染和窗口
> shell；`Widget` trait 把 layout、draw、update、overlay、operate、diff 分为生命周期钩子；0.14 新增 headless testing。它不是 headless behavior
> primitive：行为、布局、绘制、style 和 renderer trait 往往同在 widget 实现内。open-gpui 应吸收 crate 分层和 headless testing，但进一步拆出 renderer-
> neutral 行为状态机、AccessKit metadata、positioning service 和 style recipe。

### 渲染与性能

#### `rendering_model`

> Iced 是 native retained-ish widget tree 加自绘渲染模型：view 产生 Element/widget tree，runtime 处理事件、diff/layout，再通过 renderer 绘制。默认提供
> wgpu GPU 渲染和 tiny-skia 软件渲染，可在 native winit shell 和 Web canvas/WebGPU/WebGL 场景运行；不是 DOM/WebView，也不是 egui 式纯 immediate
> mode。

#### `performance_model`

> Iced 的性能策略包括 renderer abstraction、wgpu GPU 后端、tiny-skia fallback、响应式渲染、widget tree diff、lazy widget、primitive
> culling、graphics layer merging、texture/geometry cache、并发 image decoding/uploading、文本 shaping 与缓存、scrollable 改进、headless
> 渲染和 performance metrics/devtools。大列表/大表格方面 0.14 加入 table/grid，但公开资料未证明其虚拟化、百万行、树表、复杂文本编辑性能已达到 Zed/GPUI 级别。open-gpui
> 应参考其工具链，但把性能优势押在文本、表格、树、低延迟输入和场景图增量渲染。

#### `native_advantage`

> Iced 相对 WebView/DOM 的优势在 Rust 单语言栈、无浏览器嵌入、GPU/软件渲染可控、跨平台窗口 shell、可嵌入现有 wgpu 应用、类型安全、低层自定义 widget、Canvas/Shader、统一
> Task/Subscription 和 headless testing。open-gpui 应进一步在大文本/代码编辑、虚拟列表/树/表、窗口级 overlay、多窗口多显示器、精确输入延迟、GPU scene 和 AccessKit
> 上形成比 Iced 更强的 native 差异化。

#### `web_ecosystem_advantage`

> Web/Tauri/Electron 生态仍在组件数量、ARIA/浏览器辅助技术、CSS/tokens、动画库、图表、表单、Storybook/Chromatic、可视化调试、远程
> registry、设计系统工具和第三方插件上更成熟。Iced 的 Web 目标是 canvas/WebGPU/WebGL，不继承 DOM 生态。open-gpui 不应追完整 Web 生态覆盖面，而应提供与 WebView/导出工具/设计
> token pipeline 的互操作，并优先做 native 桌面强项。

### 主题与设计系统

#### `theme_token_model`

> Iced 有 first-class `Theme`、`Palette`、widget style/catalog、系统主题反应、light/dark 示例和 per-widget style 方法；0.14 对 palette
> generation 做了 Oklch 方向的改进，并支持系统主题检测。它不是 DTCG token schema，也没有公开的语义 token registry、fallback/modes/states/variants
> manifest。open-gpui 可参考强类型 theme 和 widget style trait，但应把 semantic tokens、component slots、state
> variants、density、motion、fallback 和 schema version 显式化。

#### `style_customization_boundary`

> Iced 的边界是 framework 提供基础 Theme、Palette、widget style 类型和默认样式；组件 prop/builder 方法提供局部样式和布局配置；用户应用可以自定义 Theme、style closure
> 或自定义 Widget；renderer 负责把 primitive 画出来。这个边界对 Rust 应用框架自然，但对 open-gpui 通用组件生态还不够：需要进一步区分 core behavior、theme
> recipe、component prop、app adapter 和用户源码 ownership。

### 组件表面

#### `component_coverage`

> 覆盖度中高：基础布局和控件包括
> button、checkbox、radio、slider、vertical_slider、toggler、text、rich_text、text_input、text_editor、container、row、column、grid、stack、scrollable、rule、space、responsive、mouse_area；overlay/选择包括
> tooltip、pick_list、combo_box、float、overlay；数据/应用结构包括 table、pane_grid、progress_bar；媒体和高级能力包括
> canvas、image、svg、qr_code、markdown、shader、lazy、sensor、selector 等。缺少成熟通用 dialog/menu/tabs/tree/command palette/form
> validation/date picker 等完整设计系统面。

#### `must_have_for_open_gpui`

> 必须借鉴的是 Elm-style 清晰状态模型、Task/Subscription 副作用边界、renderer/runtime/widget/window crate 分层、feature flags、Program/Widget
> trait 生命周期、headless testing、E2E recorder、snapshot、debug/time-travel/performance metrics、examples-as-crates 和 Cargo-
> native 分发。对 open-gpui 通用 UI 框架，首批应补
> Dialog、Popover、Menu、Tooltip、Select/Combobox、Tabs、Checkbox/Radio/Switch/Slider、TextInput、ScrollArea、Table/Tree、Toast，以及统一
> focus、overlay positioning、AccessKit、theme token 和 contract tests。

#### `do_not_chase`

> 当前阶段不应追 Iced 的完整应用框架外壳、Web canvas target、所有 renderer 后端、time-travel/debugger 全量产品化、hot reloading、桌面 shell
> 细节、Markdown/QR/Image/Shader 等长尾 widget，也不应复刻 Iced 的所有 widget API。open-gpui 更应追通用 primitive contract、GPUI-native
> Element/Entity 适配、AccessKit、overlay、theme token、gallery 和测试门禁。

### 文档测试工具

#### `docs_gallery_model`

> Iced 有官网、book、docs.rs、examples 目录、showcase、多媒体示例和 demo；examples 以 Cargo package 可运行，Tour 示例集中展示
> state/message/update/view 和控件。0.14 的 test/devtools 让文档与验证更接近，但公开资料未显示 docs、gallery、examples、schema、截图、a11y contract 和
> scaffold 由同一事实源生成。open-gpui 应把 Iced 的 examples-as-crates 升级为 component gallery + machine-readable manifest +
> screenshot/interaction/a11y tests。

### 治理

#### `versioning_and_breakage`

> 项目使用 changelog 并声明遵循 SemVer，但 README 也明确标注 Iced 仍是 experimental software，examples README 提醒 master 分支可能包含 breaking
> changes。0.x 阶段 API 演进快，0.14 引入大量新增和变更；用户需要锁版本或使用 latest 分支示例。open-gpui 若要对外做通用组件框架，应比 Iced 更早定义 public API
> surface、deprecated 生命周期、migration guide、schema version 和 compatibility tests。

#### `maintenance_cost`

> 维护成本高：Iced 同时维护核心抽象、runtime、widget 库、wgpu/tiny-skia renderer、winit shell、Web 支持、文本、异步、debug/devtools、test
> recorder、examples、feature flags 和跨平台 bug。收益是生态闭环完整，但对 open-gpui 来说直接追同等范围会稀释 GPUI
> 的核心优势。更现实的策略是只吸收少数高杠杆基础设施：测试/调试、renderer-neutral contract、overlay/focus、theme、核心 widgets。

#### `risks`

> 主要风险是把 Iced 当成 open-gpui 组件框架蓝图而忽视目标差异：Iced 是完整应用框架，open-gpui 更需要 GPUI 上层通用 primitive 和组件生态。第二个风险是复制其 widget 内聚方式后缺少
> headless 行为 contract、AccessKit 和 anatomy，导致复杂组件不可组合不可验证。第三个风险是追 Web/Wasm/renderer 多后端、devtools、热重载、长尾
> widget，拖慢原生桌面核心能力。第四个风险是 accessibility 仍不成熟，不能作为 open-gpui 的可访问性设计依据。

#### `open_gpui_relevance`

> 建议为 reference-only 加 targeted adopt。不要采用 Iced 作为 open-gpui 的架构母体，也不要复制其完整应用框架；应定向吸收 Rust/Cargo 分发、Elm-style
> 状态边界、Task/Subscription、widget tree diff、renderer/runtime 分层、headless simulator、E2E recorder、snapshot、debug/performance
> metrics 和 examples 组织。直接设计含义是 open-gpui 应先定义 GPUI-native primitive contract：typed state machine、part anatomy、AccessKit
> mapping、overlay geometry、theme token、gallery manifest 和验证工具；Iced 可作为 Rust-native framework/tooling 标杆，而不是 headless
> component API 标杆。

### 不确定字段（已跳过）

- `accessibility_model`
- `ai_friendliness`
- `component_anatomy_model`
- `design_token_pipeline`
- `diagnostics_and_failure_quality`
- `interaction_state_machines`
- `machine_readable_contracts`
- `positioning_and_collision_model`
- `registry_viability`
- `testing_strategy`
- `third_party_ecosystem_path`

## <a id="egui"></a>17. egui

- 结果文件：`egui.json`
- 调研类别：`rust_immediate_mode_ui`
- 纳入原因：
  Rust immediate-mode GUI 代表；适合研究工具型 UI、低摩擦 API、调试/inspector 生态，但也要识别不适合复杂 native app 的边界。
- 参考来源：
  - https://www.egui.rs/

### 定位

#### `positioning`

> egui 是 Rust immediate-mode GUI library，核心定位是“简单、快速、可移植、易嵌入”的 2D 自绘 UI 库；`eframe` 是官方应用框架，负责 web/native 的窗口、输入和渲染集成。它不是
> headless primitive 库，也不是完整 native design system，更像“Dear ImGui 风格的 Rust-native 工具型 UI 核心 + 官方集成层 +
> demo/testing/inspection 生态”。

#### `target_users`

> 主要服务 Rust 应用开发者、调试/inspector/内部工具作者、游戏引擎集成者、数据可视化产品团队、需要低样板 UI 的个人项目和希望同一套代码跑 native/Web/Wasm 的团队。对 open-gpui
> 最相关的用户画像是桌面工具、开发者工具、debug 面板、gallery 和 AI 可驱动验证工具作者，而不是追求复杂原生应用壳的设计系统团队。

#### `primary_value_proposition`

> 核心价值是把 UI 写成普通 Rust 函数：应用状态作为可变数据传入，控件立即返回 `Response`，无需回调、对象生命周期和 retained tree 管理。它与 open-gpui 的匹配点在于低摩擦
> API、自绘渲染、可嵌入、测试/inspection 工具链和调试体验；不匹配点是 immediate mode 对复杂布局、稳定组件 anatomy、细粒度增量渲染、成熟设计系统和大型 native app 可访问性治理的天然边界。

### 分发与生态

#### `distribution_model`

> 分发以 Cargo package dependency 为主，核心 crate 包括 `egui`、`eframe`、`egui-winit`、`egui-
> wgpu`、`egui_glow`、`egui_extras`、`egui_kittest`、demo 和相关集成。重依赖被放到 `eframe`、renderer 或 `egui_extras`，`egui` 本体保持平台无关和
> Wasm-friendly；模板主要是 `eframe_template`，示例来自仓库 examples、web demo 和 demo app；第三方组件通过 crates.io、GitHub wiki 和社区 crate
> 传播。它没有官方源码 registry、CLI add、copy-to-own 组件模型或插件市场。

#### `source_ownership`

> 用户拥有自己的 app state、UI 函数、自定义 widget、style 调整和集成代码；egui/eframe/renderer 默认作为上游 crate 依赖使用。MIT/Apache-2.0 许可允许 fork 或
> patch，但升级会受上游 API 演进影响；官方文档明确说明 egui 仍在活跃开发且新版本会有 breaking changes。相比 copy-to-own 组件库，egui 的业务 UI 更容易本地修改，但内置 widget
> 行为、布局和渲染细节仍是依赖包边界。

#### `rust_distribution_fit`

> 适配度很高。egui 的 crate 拆分、feature flags、docs.rs、Rustdoc JSON、Cargo examples、`eframe_template`、Wasm/native 双目标、`wgpu`/`glow`
> 后端选择、`egui_extras` 承载重依赖、`egui_kittest` 承载测试工具，都符合 Rust 生态的渐进式依赖模型。对 open-gpui 的启发是核心 crate
> 要轻，renderer/window/testing/extras 要分层，重型能力通过 feature 或独立 crate 承载，并用 examples 和 docs.rs 降低采用成本。

### AI 时代设计

#### `copy_modify_verify_loop`

> egui 的典型闭环是 `cargo add egui/eframe` 或从 `eframe_template` 起步，在普通 Rust 函数里组合
> `ui.label`、`ui.button`、`ui.add(Slider...)`、`Window`、`Panel`、`ScrollArea` 等控件，必要时实现自定义 widget 或用 `Painter` 绘制，再通过 `cargo
> check/test/run`、web/native demo、`egui_kittest` 角色/标签查询、交互驱动、snapshot、AccessKit inspector 和 `egui_mcp`/inspection
> 协议验证。它不是复制官方组件源码再本地拥有的模型；open-gpui 可借鉴其短反馈循环，但要补 schema/visual/a11y/performance gate。

### API 与组合

#### `api_ergonomics`

> API 形态非常低摩擦：先有 `Context`，再通过 `Window` 或 `Panel` 拿到 `Ui`，在闭包里按顺序调用控件；控件返回 `Response`，例如 `if ui.button("保存").clicked() {
> ... }`。复杂控件使用 builder pattern 和 `ui.add(...)`，布局使用 `horizontal`、`vertical`、`columns`、`collapsing` 等闭包，绘制使用 `Painter` 和
> shape，集成层每帧输入 `RawInput`、运行 UI、处理 `FullOutput`、tessellate 并渲染三角网格。代价是 API 把布局、交互、绘制和状态访问放在同一帧代码流里，复杂组件的公共契约不如
> retained/headless 系统清楚。

#### `customization_model`

> 定制主要通过 `Context::set_style`、`Style`、`Visuals`、spacing、fonts、sizes、widget builder 参数、`Painter` 自绘、自定义 `Widget` 和
> renderer/integration 扩展完成。0.35.0 引入 `Classes`，可在 `UiBuilder` 或部分 widget 上设置 class，让自定义 widget 根据上下文调整行为或样式，并为后续 CSS-like
> styling 打基础。当前样式能力仍不是 CSS 或 design token pipeline；对 open-gpui 来说，应借鉴“低样板 escape hatch”，但把 theme token、component
> slots、state variants 和 app adapter 边界做得更显式。

#### `state_ownership_model`

> 应用业务状态由用户持有并在每帧传入 UI 函数；egui 只保留少量 GUI memory，例如窗口位置/尺寸、滚动位置、折叠状态、拖拽中的 widget、焦点和与 `Id` 相关的临时状态。用户需要在动态列表、重复窗口或持久状态处提供稳定
> ID，`persistence` feature 可序列化部分 memory。整体是 application-owned state + framework-owned ephemeral UI memory 的混合模型，没有 React
> 式 controlled/uncontrolled 命名，但边界清楚：业务真源归应用，短期交互连续性归 egui memory。

### Headless 与行为

#### `headless_boundary`

> egui 本体相对平台无关：它不负责 OS、输入采集或最终上屏，集成层提供 `RawInput` 并处理 `FullOutput`，renderer 负责绘制 tessellated primitives；`eframe`、`egui-
> winit`、`egui-wgpu`、`egui_glow` 则提供官方集成。这个边界利于嵌入游戏引擎和多后端，但 egui 不是 headless behavior primitive：多数 widget
> 的行为、布局、绘制、样式和可访问信息在同一 immediate-mode 调用或 widget 实现中完成。open-gpui 应参考平台/renderer 分层，而不要照搬行为与渲染强耦合的组件边界。

### 渲染与性能

#### `rendering_model`

> egui 是纯 immediate-mode 自绘模型：每帧运行 UI 代码、布局、处理交互、收集 shapes，随后 tessellate 为 clipped primitives/triangle meshes，由 `egui-
> wgpu`、`egui_glow` 或其他 integration 渲染。它不是 DOM/WebView，也不是 retained native widget；可以嵌入任何能绘制 textured triangles
> 的环境，包括游戏引擎、native window 和 Web/Wasm canvas/WebGPU/WebGL 场景。

#### `performance_model`

> 性能策略是让每帧全量 UI/layout 足够轻、只在交互或动画时 repaint、保持核心依赖少、必要时使用 cache、并通过 `rayon` feature 支持 parallel tessellation。官方目标包括 debug
> build 60Hz，README 也指出大多数场景可期待 egui 每帧约 1-2ms，但巨大 scroll area 或长 scrollback 会因为每帧 layout 变慢；推荐只 layout 可见部分，`egui_extras`
> 和社区虚拟列表/table crate 也在补这类能力。对 open-gpui 来说，egui 的性能模型适合中小型工具 UI，不适合作为大文本、大表格、大树和复杂应用壳的长期性能蓝图。

#### `native_advantage`

> egui 相对 WebView/DOM 的优势在低启动和集成成本、Rust 单语言栈、无浏览器嵌入、可嵌入渲染循环、GPU/GL 后端可控、游戏/3D overlay 友好、custom painting 直接、debug
> 面板和数据可视化交付快。open-gpui 应在这些优势上更进一步：用 retained/增量模型、GPU scene、文本/表格/树虚拟化、窗口/焦点/AccessKit 一体化和低延迟输入，形成比 egui 更适合复杂 native
> productivity app 的差异化。

#### `web_ecosystem_advantage`

> Web/Tauri/Electron 在成熟 CSS layout、ARIA/浏览器辅助技术、组件数量、Storybook/Chromatic、表单、富文本、图表、地图、token pipeline、动画和第三方 SDK
> 上仍明显更强。egui 的 Web 目标是 Wasm + canvas/WebGPU/WebGL，不继承 DOM 组件生态。open-gpui 不应追完整 Web 生态覆盖面，而应提供 Web 心智可迁移的 component
> anatomy、theme token、gallery 和必要 WebView/导出互操作，同时把性能优势压在 native 桌面强项。

### 主题与设计系统

#### `theme_token_model`

> egui 主题模型以 `Style`、`Visuals`、fonts、spacing、sizes、widget states 和 runtime `Context::set_style` 为主，并在 0.35.0 开始引入
> `Classes` 支持上下文相关 styling。它没有成熟的 semantic token、component token、mode、density、motion、fallback、schema version 或 design-
> tool token 文件模型。open-gpui 可参考其强类型 style 与即时预览体验，但应把 token key、状态变体、组件 part、fallback 和版本迁移显式化。

#### `style_customization_boundary`

> egui 的样式边界偏应用内：framework 提供默认 `Style/Visuals` 和控件行为，用户通过 context style、控件 builder、class、scope、custom widget 和 painter
> 改视觉，renderer 只负责画 primitives。这个边界让局部工具 UI 很快，但大型组件生态需要更细分：core behavior 不应依赖视觉，theme recipe 只消费状态和 token，component prop
> 处理变体，用户源码替换 parts，app adapter 处理平台策略。open-gpui 应吸收 egui 的 escape hatch，但避免样式、结构和行为揉成不可验证的 widget 函数。

### 组件表面

#### `component_coverage`

> 内置覆盖基础控件和工具型 UI：label、button、hyperlink、checkbox、radio、slider、drag value、text edit、color
> picker、spinner、image、layout、columns、wrapping、window、panel、scroll area、collapsing header、tooltip、context menu、menu、combo
> box、progress 等；`egui_extras` 和社区提供 table、markdown、date、file dialog、dock、virtual list、toast、map、form、JSON tree、plot
> 等。覆盖面足以构建工具和专业应用原型，但不是完整设计系统级的 dialog/menu/select/tabs/tree/table/form validation/application shell/rich text 统一组件面。

#### `must_have_for_open_gpui`

> 对 open-gpui 必须借鉴的是低摩擦 Rust API、可嵌入 renderer 边界、custom widget/painter 体验、web demo/gallery 的可试用性、AccessKit-backed
> testing、snapshot 测试、inspection protocol、agent 可操作 app 的 `egui_mcp` 思路、callstack/debug/introspection 这类定位 UI
> 来源的能力。落地时应转译为 GPUI-native component contract：part anatomy、typed state machine、AccessKit mapping、overlay geometry、theme
> token、gallery manifest 和 visual/interaction/performance gates。

#### `do_not_chase`

> 当前阶段不应追 egui 的完整 immediate-mode 应用模型、游戏引擎集成广度、Wasm canvas parity、所有 renderer 后端、egui 默认视觉风格、社区 widget 全量覆盖、用 wiki
> 管理生态、或把复杂 native app 的主界面建立在每帧全量 layout 的模式上。open-gpui 更应聚焦 GPUI 的 retained/entity/element
> 优势、高价值桌面控件、AccessKit、overlay、theme token、测试门禁和稳定组件 API。

### 文档测试工具

#### `testing_strategy`

> egui 当前最值得参考的是 `egui_kittest`：它基于 AccessKit/kittest，可用 role/name 查询控件，执行 click/type/key 等交互，运行 frame，压缩窗口到内容大小，并在启用
> `wgpu` 和 `snapshot` feature 后进行图像快照测试；`kittest.toml` 可配置阈值和 snapshot 输出。再加上 Rust 单元测试、examples、demo、AccessKit
> inspector、0.35 inspection protocol 和 `egui_mcp`，已经形成较先进的 Rust GUI 测试闭环。open-gpui 应吸收这套思路，并补 overlay geometry、state
> machine、theme token、API surface、schema drift 和大数据性能测试。

### 治理

#### `versioning_and_breakage`

> egui 使用 Cargo/crates.io 发布，当前官方 release 为 0.35.0（2026-06-25），MIT OR Apache-2.0 许可。项目仍处于活跃开发和 0.x 阶段，官方文档明确说明接口仍会变化、新版本会有
> breaking changes，README 也提示如果想要不会升级破坏的 GUI，egui 目前还不适合。open-gpui 若面向通用组件生态，应比 egui 更早定义稳定 API surface、deprecated
> 生命周期、migration guide、schema version 和 compatibility tests。

#### `maintenance_cost`

> 维护成本中高。immediate-mode 降低了 widget 生命周期和应用 API 复杂度，但 egui 仍要维护核心
> UI、布局、文本、绘制/tessellation、AccessKit、memory/ID、Wasm/native、多 renderer、多 integration、testing、inspection、demo、examples 和大量
> edge cases。对 open-gpui 来说，直接 adopting egui 式全栈会稀释 GPUI 的 retained/native 优势；更现实的是只吸收低摩擦 API、testing/inspection、debug
> tooling 和部分 custom painting 经验。

#### `risks`

> 主要风险是把 egui 的工具型 immediate-mode 成功误判为复杂 native app 框架蓝图。Immediate mode 的 layout paradox、每帧全量 layout、巨大 scroll area 性能、ID
> 稳定性、样式系统弱、非 native look、API breaking、复杂组件无 anatomy、a11y 语义易遗漏和第三方 crate 治理松散，都会在 open-gpui 目标场景里放大。AI 生成 egui
> 风格代码很快，但如果没有 contract/test gate，容易产生看似可运行、长期不可维护的 UI。

#### `open_gpui_relevance`

> 最终建议：reference-only + targeted adopt。不要采用 egui 作为 open-gpui 通用组件架构母体，也不要把主组件框架改成 immediate mode；应定向吸收它的低样板调用体验、custom
> widget/painter escape hatch、Cargo 分层、web demo/gallery、AccessKit-backed kittest、snapshot、inspection protocol、`egui_mcp` 和
> debug/introspection 思路。直接设计含义是：open-gpui 应保持 GPUI-native retained/entity 模型，同时提供 egui 式简洁 authoring layer，并用 machine-
> readable component contract、AccessKit、overlay geometry、theme token 和测试门禁补齐 egui 的长期边界。

### 不确定字段（已跳过）

- `accessibility_model`
- `ai_friendliness`
- `component_anatomy_model`
- `design_token_pipeline`
- `diagnostics_and_failure_quality`
- `docs_gallery_model`
- `interaction_state_machines`
- `machine_readable_contracts`
- `positioning_and_collision_model`
- `registry_viability`
- `third_party_ecosystem_path`

## <a id="xilem-linebender-ui"></a>18. Xilem / Linebender UI

- 结果文件：`Xilem_Linebender_UI.json`
- 调研类别：`rust_reactive_native_ui_experiment`
- 纳入原因：
  > Rust native reactive UI 与 masonry/xilem 架构探索；适合比较 data model、view reconciliation 和 renderer-neutral design。
- 参考来源：
  - https://github.com/linebender/xilem

### 定位

#### `positioning`

> Xilem / Linebender UI 是 Rust 原生 reactive UI 架构实验，同时包含高层 Xilem framework、底层 Masonry retained widget toolkit、Xilem Core
> renderer-neutral reactivity primitives、Masonry Core GUI engine、Masonry Testing headless harness 和 Xilem Web DOM
> backend。它不是 shadcn 式组件 registry，也不是纯 headless primitive；更像“React/SwiftUI/Elm 式 view reconciliation + Masonry retained
> widget tree + Vello/wgpu/Parley/AccessKit 基础设施”的研究型原生 UI 框架。

#### `target_users`

> 主要服务 Rust GUI 框架作者、愿意接受 alpha 风险的 Rust 应用开发者、想研究 reactive native UI 的架构师、需要 retained widget tree/testing/accessibility
> 基础设施的库维护者，以及 Linebender 生态贡献者。

#### `primary_value_proposition`

> 核心价值是把短生命周期、强类型 view tree 与长期存在的 element/widget tree 分离，通过 `build/rebuild/message` 做最小更新，并把原生窗口、2D 渲染、文本、无障碍和测试能力沉到
> Masonry。它与 open-gpui 的匹配点很强：data model、view reconciliation、renderer-neutral core、retained tree pass
> system、AccessKit、headless test harness 都值得参考；但它仍是 alpha 实验框架，不适合作为 open-gpui 组件 API 或生态分发模型的直接蓝图。

### 分发与生态

#### `distribution_model`

> 分发以 Cargo package dependency 和 workspace crate 为主。顶层建议 `cargo add xilem`，实际拆分为
> `xilem_core`、`xilem_masonry`、`xilem`、`xilem_web`、`masonry_core`、`masonry`、`masonry_testing`、`masonry_winit`、`masonry_imaging`、`tree_arena`
> 等 crate；示例在仓库内通过 `cargo run --example ...` 运行。没有官方 CLI add、copy-to-own registry、component marketplace 或源码 recipe
> registry。feature flags 主要用于默认 Masonry/Winit、testing、tracy 和截图渲染后端等能力。

#### `source_ownership`

> 用户通常通过 crates.io 或 git dependency 消费框架源码，不拥有内置 widget 的本地副本；可以 fork、patch 或实现自定义 Masonry widget/Xilem view，但升级成本取决于 0.x
> alpha API 的变化。应用状态、component 函数、view 组合和自定义 widget 归用户所有；Xilem 的 `ViewState`、`ViewSequence::SeqState` 等内部状态明确不应视为公共
> API，甚至 patch release 也可能变化。

#### `rust_distribution_fit`

> 适配度很高。Xilem 使用 Rust 2024 edition、Cargo workspace、crates.io crate、feature flags、MSRV 声明、docs.rs、cargo-rdme、严格
> lint、Apache-2.0 许可和 `cargo add xilem` 的常规 Rust 分发路径。其 crate 分层对 open-gpui 有直接启发：应拆清 core contract、native backend、test
> harness、examples/gallery、window integration 和可选调试/性能功能，避免单一巨型 crate 暴露过多不稳定类型。

### AI 时代设计

#### `copy_modify_verify_loop`

> 常规闭环是 `cargo add xilem` 或依赖 Masonry crate，复制官方 examples 的模式，编写 `app_logic(data) -> impl WidgetView`、组合 view 函数或实现自定义
> Masonry `Widget`，再用 `cargo check/test/run`、Masonry `TestHarness`、模拟鼠标/键盘/文本/AccessKit
> 事件、`assert_render_snapshot`、示例截图、F11 widget inspector、F12 layout debug、tracing/tracy 和 divan bench 验证。它不是 copy-to-own
> 组件源码模型；open-gpui 可借鉴“生成/修改后用编译 + headless interaction + screenshot + access tree”验证，而不是照搬其分发方式。

### API 与组合

#### `api_ergonomics`

> Xilem API 以强类型声明式 composition 为主：应用状态是任意 `'static` Rust 类型，根 `app_logic`/component 函数根据 `&mut AppState` 返回轻量 view
> tree；按钮等事件闭包可直接拿到 `&mut AppState` 修改数据；下一轮重新生成 view tree 后与旧 view tree diff，最小更新 Masonry widget tree。核心 trait 是
> `View::build/rebuild/teardown/message`，序列由 `ViewSequence` 管理，状态适配有 `lens`/`map_state`，性能裁剪有 `memoize`/`frozen`，动态 UI 有
> `AnyView`/`one_of`，异步有 `task`/`worker`。Masonry 低层 API 则是 `Widget` trait、driver/action、`WidgetMut`、properties 和 pass
> contexts，能力强但复杂度明显更高。

#### `customization_model`

> 自定义分两层：Xilem 层通过函数式 view 组合、闭包、state lens、memoize、AnyView、task、worker 和 custom view 接入；Masonry 层通过自定义
> `Widget`、`NewWidget`、`WidgetPod`、`WidgetMut`、`PropertySet`、`DefaultProperties`、`PropertyStack`、selector、class/status、default
> theme 和 paint/layout/accessibility 方法接入。样式主要通过 typed properties 和默认 property set，而不是 CSS 或 token 文件。escape hatch
> 较多，但需要理解 Masonry pass system，复杂组件的局部替换和 slot 化能力还不是 Radix/Ark 风格。

#### `state_ownership_model`

> 应用状态由用户的 Rust 类型集中拥有，Xilem 在消息处理时把 `&mut AppState` 暴露给 view 回调；view tree 是短生命周期、用于 diff 和消息路由的描述；element/widget tree 由
> Masonry retained；每个 `View` 的 `ViewState` 保存消息路径、子 view 状态或 memoized view 等内部信息；Masonry widget
> 自己拥有交互、布局、焦点、滚动、文本和属性状态。整体是 application-owned state + framework-retained widget state + renderer/backend-owned element
> state 的混合模型，不是 Web 里 controlled/uncontrolled props 的直接命名体系。

### Headless 与行为

#### `headless_boundary`

> 边界设计很有价值但不完全 headless。`xilem_core` 抽象 renderer-neutral reactivity，`xilem_masonry` 和 `xilem_web` 把同一模式接到 Masonry widget
> tree 或 DOM；`masonry_core` 提供事件、布局、compositing、AccessKit、WidgetMut 和 Action；`masonry` 提供具体 widgets/properties/default
> theme；`masonry_winit` 接窗口。Masonry 的 pass system 将 pointer/text/access
> events、mutate/update/layout/compose/paint/accessibility 分层清楚。但具体 widget 往往同时包含行为、布局、绘制、无障碍和样式读取，因此它不是 Radix/Zag 那种
> renderer-neutral behavior state machine 库。

### 渲染与性能

#### `rendering_model`

> native 端是 reactive lightweight view tree + retained Masonry widget tree + 自绘 2D 渲染模型。Xilem 生成 view tree，diff/rebuild 后更新
> Masonry element/widget tree；Masonry 通过事件、rewrite、layout、compose、paint、accessibility passes 维护树，并使用 Imaging/Vello/wgpu 等
> 2D 渲染基础设施、Parley/Fontique 文本栈、AccessKit 无障碍和 winit 窗口。另有 `xilem_web` 把同一核心模式接到 DOM，但 native Xilem 本身不是 WebView/DOM。

#### `performance_model`

> 性能策略包括强类型 view tree diff、短生命周期轻量 view、retained widget tree 最小更新、`memoize`/`frozen` 跳过子树重建、Masonry invalidation flags 和
> rewrite passes、layout 与 compose 分离、Vello/wgpu/Imaging 渲染、Parley 文本、AccessKit pass、VirtualScroll MVP、widget_list divan
> benchmarks、snapshot tests 和 tracing/tracy。风险是项目明确 alpha，VirtualScroll 文档列出 focus、完整 a11y、scrollbar、touch
> gesture、transform 等 caveat；大表格、大树、复杂富文本和生产级编辑器性能还没有被公开资料证明。

#### `native_advantage`

> 相对 WebView/DOM，Xilem/Masonry 的 native 优势在于 Rust 单语言栈、强类型 view 和 app state、无浏览器嵌入、可控 retained tree、GPU 2D 渲染、Parley
> 文本、AccessKit 树、窗口/事件/IME/focus 的底层可控性、headless test harness 和低层自定义 widget。open-gpui 更应把这些优势用于大文本/代码编辑、虚拟列表/树/表、低延迟输入、精确
> overlay/focus、GPU scene、截图/AccessKit 测试和可诊断 pass system。

#### `web_ecosystem_advantage`

> Web/Tauri/Electron 生态在组件数量、ARIA battle testing、CSS/layout 动画、设计 token 工具、图表、表单、Storybook/Chromatic、浏览器 devtools、远程
> registry、AI 训练语料和第三方插件上明显更成熟。Xilem Web 证明 Xilem Core 可接 DOM，但这不等于继承 Web 组件生态。open-gpui 不应追求复刻完整 Web 组件库，而应在 native
> 强项上建立差异，并保留 token、文档、WebView、导出和设计工具互操作。

### 主题与设计系统

#### `theme_token_model`

> Masonry 有默认 theme 常量、`DefaultProperties`、`PropertySet`、`PropertyStack`、typed
> `Property`、selector/class/status、hover/focus/active/disabled 等状态选择和 per-widget 默认属性；常见样式如
> Background、BorderColor、BorderWidth、Padding、ContentColor、CornerRadius、TextInput caret/selection/placeholder、Slider
> thumb/track 等都通过属性读取。它不是 DTCG token/schema/theme file 体系，也没有模式、语义 token、fallback、density、motion、component slots 的稳定
> manifest。

#### `style_customization_boundary`

> 样式边界在 Masonry 中是：framework 定义 property 类型、selector/status 计算和默认 property set；widget paint/layout/accessibility 代码决定读取哪些
> properties；应用或上层框架通过 `NewWidget`/`WidgetMut`/PropertySet/PropertyStack 设置局部属性；Xilem view 可包装 style/prop view 把属性传给
> Masonry。这个模型简单且 Rust-native，但属性设置到不读取该属性的 widget 上不会生效且可能无告警。open-gpui 应保留 typed style props 的安全感，同时补组件 slot/token 合同和无效
> token/part 诊断。

### 组件表面

#### `component_coverage`

> 覆盖度偏基础到中等。Masonry/Xilem 具备 label/prose/text input/text area、button/text_button、checkbox、radio/radio
> group、switch、slider、progress bar、spinner、badge、divider、image/svg/canvas、flex/grid/sized_box/zstack/split/indexed
> stack、portal/scrollbar、selector、pagination、step input、collapse panel、virtual scroll、resize observer、task/worker 等。缺少成熟通用
> dialog、popover、menu、dropdown/select 完整组合、tabs、combobox、toast、tree、table/data grid、command palette、form validation、date
> picker、rich application shell 等设计系统表面。

#### `must_have_for_open_gpui`

> 必须吸收的是架构思想而非完整 API：轻量 view tree 与 retained element tree 分离、`build/rebuild/message` 式 reconciliation、Xilem Core/backend
> 分层、Masonry pass system、layout/compose 分离、AccessKit tree pass、typed properties/default property stack、TestHarness
> 交互模拟、screenshot snapshot、debug inspector/layout overlay、tracing/tracy 和 examples-as-tests。对 open-gpui 组件框架来说，还必须补 Xilem
> 没有成熟覆盖的 Dialog、Popover/Menu/Tooltip/Select、Tabs、TextInput、Form controls、Table/Tree/Virtualized
> list、Toast、FocusScope、Overlay positioning、AccessKit contract、theme token schema 和 component manifest。

#### `do_not_chase`

> 当前阶段不应追 Xilem 的完整跨后端框架野心、Xilem Web DOM backend、所有 Masonry widget、Android/winit driver 细节、两进程 hot reload 设想、Imaging/Vello
> 后端抽象全量、Masonry 独立应用框架体验或 Linebender 生态的所有基础设施。open-gpui 应避免被“重建一个新 Xilem”分散目标，而应选择性吸收
> reconciliation、pass/test/a11y/theme contract 这些能强化 GPUI-native 组件生态的部分。

### 文档测试工具

#### `testing_strategy`

> 测试策略是 Xilem/Masonry 最值得借鉴的部分之一。`masonry_testing` 提供 `TestHarness`，可模拟 mouse movement/click、keyboard、text
> input、IME、AccessKit actions、时间/动画、scroll into view，能读取 widget tree/access tree、pop actions、编辑 widget，并用
> `assert_render_snapshot` 做截图回归；Masonry 文档强调 widget/unit/example/screenshot tests，VirtualScroll/Portal 等有行为和截图测试；还有 divan
> benchmark、tracing/tracy、严格 lint 和 debug assertions。缺口是还没有看到面向所有组件的统一 API surface、schema drift、import boundary、a11y
> coverage matrix 和性能预算门禁。

### 治理

#### `versioning_and_breakage`

> 版本治理需要谨慎。官方 README 明确 Xilem 当前是 alpha/experimental；workspace 当前标 `0.4.0`，Rust edition 2024，当前主干 README 声明 MSRV 1.92，且未来
> MSRV 提升不视为 breaking change；`ViewState`/`SeqState` 等内部状态明确不作为公共 API，patch release 也可变化。对应用或 open-gpui 研究来说应 pin 版本或
> commit，并把它视为设计参考和实验依赖，而不是稳定公共组件生态的先例。

#### `maintenance_cost`

> 维护成本很高：Xilem/Masonry 同时维护 reactivity core、native backend、web backend、retained widget
> engine、layout/event/focus/IME/accessibility passes、winit integration、2D renderer/text/accessibility dependencies、基础
> widgets、properties/default theme、testing/snapshot/debug/profiling、examples 和文档。open-gpui 若全量追随会吞掉大量资源；更合理的是复用其思想，优先建设
> GPUI 已经需要的 component contract、overlay/focus/a11y、theme token 和 test harness。

#### `risks`

> 主要风险是把一个 alpha reactive UI framework 当成成熟组件库参考，导致 open-gpui 过度追求 framework completeness 而忽略通用组件 primitive。第二个风险是照搬
> Masonry widget 内聚方式后，行为、布局、绘制、a11y、样式仍耦合在 widget 中，缺少 AI 可验证的 headless 状态合同。第三个风险是追跨后端、Web、Android、renderer abstraction
> 和长尾 widgets，稀释 GPUI 原生性能优势。第四个风险是当前 overlay collision、复杂 a11y、组件 anatomy、token pipeline 和第三方生态都不成熟，需要 open-gpui 自行补设计。

#### `open_gpui_relevance`

> 建议为 reference-only + targeted trial。不要 adopt Xilem/Masonry 作为 open-gpui 架构母体，也不要复制其完整 API；应定向试验三类设计：一是轻量 view contract 到
> GPUI retained entity/element 的 reconciliation；二是 Masonry 式 event/update/layout/compose/paint/accessibility pass
> vocabulary 对 open-gpui 组件测试和诊断的帮助；三是 TestHarness/screenshot/access tree/diagnostics 能否成为 open-gpui gallery 和 AI
> 修改闭环的门禁。直接设计含义是 open-gpui 应优先定义 renderer-neutral primitive contract、part anatomy、typed state machine、AccessKit
> mapping、overlay geometry、theme token schema 和机器可读 gallery/test manifest。

### 不确定字段（已跳过）

- `accessibility_model`
- `ai_friendliness`
- `component_anatomy_model`
- `design_token_pipeline`
- `diagnostics_and_failure_quality`
- `docs_gallery_model`
- `interaction_state_machines`
- `machine_readable_contracts`
- `positioning_and_collision_model`
- `registry_viability`
- `third_party_ecosystem_path`

## <a id="makepad"></a>19. Makepad

- 结果文件：`Makepad.json`
- 调研类别：`rust_native_ui_framework`
- 纳入原因：
  Rust live design、shader/native UI、cross-platform rendering 参考；适合比较 designer/developer workflow。
- 参考来源：
  - https://makepad.nl/

### 定位

#### `positioning`

> Makepad 是 Rust 原生跨平台 UI framework、GPU 渲染运行时、Live/Splash DSL、shader 样式系统、Makepad Studio/AI 开发环境和内置 widget 库的组合体。它不是
> headless primitive 库，也不是单纯组件 registry；更接近“Rust-native creative app platform + live design workflow + shader/native
> renderer”。

#### `target_users`

> 主要服务 Rust 应用开发者、需要高性能自绘 UI 的桌面/移动/WebAssembly 产品团队、UI 密集型工具团队、设计师与开发者协作场景、希望让 AI 生成可运行原生界面的团队，以及研究 shader UI、live
> coding、跨平台运行时的框架作者。

#### `primary_value_proposition`

> 核心价值是用 Rust 构建跨桌面、移动和 Web 的高性能原生 UI，同时通过 Live/Splash DSL、运行时热更新、shader 样式和 Studio 缩短设计到实现的反馈循环。它与 open-gpui 的匹配点在 GPU
> 自绘、Rust-native、复杂桌面应用、live design 和 AI 生成可验证 UI；不匹配点是 Makepad 是完整平台和 DSL-first 工作流，而 open-gpui 更需要 GPUI
> 上层通用组件、行为契约和可访问性基础设施。

### 分发与生态

#### `distribution_model`

> 分发以 Cargo/crates.io 和 GitHub workspace 为主，公开 crate 包括 `makepad-widgets`、`makepad-draw`、`makepad-platform`、`makepad-
> studio`、`cargo-makepad` 等，应用通过 Cargo 依赖或仓库示例接入；非标准目标使用 `cargo makepad` 安装 wasm、Apple、Android 等工具链；Makepad Studio 可通过
> Cargo 安装和运行；桌面打包文档推荐 `cargo-packager`，移动和 wasm 由 Makepad 工具链辅助。Live/Splash DSL 源码随应用代码进入仓库和资源包，没有看到官方 npm/shadcn 式组件
> registry、CLI add 或插件市场。

#### `source_ownership`

> 用户拥有自己的 Rust 应用代码、Live/Splash DSL、shader 片段、资源和自定义 widget 源码；框架、运行时、渲染器和内置 widget 通常作为上游 crate 依赖消费，也可以 fork MIT/Apache
> 许可源码。相比 copy-to-own 组件库，Makepad 的行为修复和渲染优化集中在上游；但如果用户深度修改内置 widget、Live 编译器或渲染后端，升级和 merge 成本会很高。对 open-gpui 来说，它证明“用户拥有
> UI 描述与业务源码，上游维护 runtime”的模型可行，但不证明源码复制型 registry 必要。

#### `rust_distribution_fit`

> 与 Rust/Cargo 适配度中高：核心能力通过 crate、workspace、feature/profile、docs.rs、cargo install 和 cargo-makepad 工具扩展；Makepad 还把 Live
> 系统、shader compiler、平台抽象和 widget 分成多个 crate/目录。代价是它引入 Live/Splash DSL、资源打包规则和自定义目标工具链，工作流比普通 Rust crate 更重。open-gpui 可借鉴
> Cargo-native 分发和 xtask/cargo 子命令思路，但应尽量让组件 contract、主题、测试和示例仍可用标准 Cargo 命令验证。

### AI 时代设计

#### `ai_friendliness`

> 中高。官方站点已经把 Makepad 定位为 AI Native Rust UIs，强调 Splash/Live DSL 可被 AI 生成、流式输出、检查和修改；Rust 类型错误和结构化诊断可约束生成结果；Studio 也强调 AI
> automation 与 UI 检查。限制是公开资料中的机器可读 contract 仍主要散落在 Rust 类型、Live AST、docs 和 examples 中，缺少类似“每个组件一份稳定 manifest”的外部事实源。open-
> gpui 应吸收它的短反馈循环和 AI 可读 UI 描述，但要补更硬的验证接口。

#### `copy_modify_verify_loop`

> Makepad 的循环是从示例或 Studio 开始，编辑 Rust 与 Live/Splash DSL，运行中的应用监听 DSL 变化并热更新 UI，必要时写自定义 widget 或 shader，再通过 `cargo
> check/run`、Makepad Studio、UI Zoo、wasm demo 和目标设备运行验证。它强调“不重编译 Rust 即可调整布局和样式”的快速设计循环。open-gpui 可借鉴 live-edit 和可视化
> inspection，但复制/生成组件后还应有 contract test、visual snapshot、AccessKit 树断言、overlay geometry test、性能基准和 AI 可读失败输出。

### API 与组合

#### `api_ergonomics`

> API 是 Rust struct/trait/宏与 Live/Splash DSL 的混合：Rust 侧通过 `#[derive(Live)]`、`#[live]`、`live_design!`、Widget
> trait、handle_event/draw/draw_walk、Action/Event/Scope、WidgetRef 和 registry 组织组件；UI 侧用节点式 DSL 描述组件树、属性、样式、布局、shader
> 和继承；绘制侧可嵌入 MPSL shader 代码。优点是样式和视觉修改很快，复杂自绘能力强；代价是开发者要理解 Rust、Live DSL、shader、Widget 生命周期和打包资源边界。

#### `customization_model`

> 定制发生在多层：Live/Splash DSL 覆盖布局、样式、属性和子节点；MPSL/shader 直接定义视觉外观；Rust widget 代码处理行为、事件、状态和自定义绘制；主题模块提供 desktop/mobile
> light/dark 等默认风格；Studio 和热更新支持快速迭代。它的 escape hatch 很强，甚至能进入 shader 和渲染层；但复杂组件的行为替换不一定像 headless parts 那样稳定，深度定制常会变成复制/改写
> widget 或理解内部 draw/event 协议。

#### `state_ownership_model`

> Makepad 是 retained widget tree 加应用状态的混合模型：Live DSL 描述可热更新的属性和结构，Rust struct 字段通过 `#[live]`/`#[rust]` 等区分参与 Live
> 应用或内部状态，Widget 处理 Event 并发出 Action，Scope/WidgetRef 支持查找和交互，draw_state/Animator/Signal/Timer 等保存运行时状态。它不采用 Web
> controlled/uncontrolled 术语；open-gpui 应从中借鉴“视觉配置可热更新，业务状态留给应用，运行时 handle 管焦点/区域/绘制”的分工，并把哪些状态可提升、可控制、可序列化写成契约。

### Headless 与行为

#### `headless_boundary`

> 边界是部分清楚但不是 headless-first：makepad-platform 负责窗口、输入、shader compiler、图形接口、网络、音视频和 Live 系统；makepad-draw 负责
> 2D/3D、layout、字体、vector、image、basic shaders；makepad-widgets 在其上提供 retained widget 和 DSL 设计能力。行为、布局、绘制、shader、样式和 a11y
> 元数据通常仍在 widget/DSL/runtime 内强耦合。open-gpui 可借鉴平台/绘制/widget 的分层，但应额外抽出 renderer-neutral behavior primitive、focus/a11y
> contract、overlay positioning service 和 theme recipe。

### 渲染与性能

#### `rendering_model`

> Makepad 是自绘 GPU-first 原生渲染模型：UI 通过 retained widget tree 和 Live DSL 组织，布局使用半 immediate/nested box/turtle draw 思路，绘制通过
> draw list、shader、2D/3D context 和平台 graphics backend 输出；目标包括
> macOS/Metal、Windows/DX11、Linux/OpenGL、Web/WASM/WebGL，以及移动平台。它不是 DOM/WebView，也不是纯 immediate mode；更像 retained UI + live
> DSL + shader/native scene pipeline。

#### `performance_model`

> 性能模型围绕 GPU 渲染、shader 样式、避免 WebView/Electron、Live DSL 热更新、DrawStep 增量绘制、draw list、geometry/text/image/vector/shader
> pipeline、portal_list/flat_list/cached_widget、profile_start/profile_end 和性能视图展开。Makepad
> 对复杂图形、2.5D、图像编辑、PDF、图表、地图、3D/glTF、XR 等场景有明确野心。公开资料未证明其大表格、大树、超大文本编辑和可访问性树性能达到生产力 IDE 级别；open-gpui
> 应把性能验证集中在文本、虚拟列表/树/表、低延迟输入、overlay 和 GPU scene 增量更新。

#### `native_advantage`

> Makepad 展示的 native 优势是无需 Electron 重包装、单 Rust 代码库、GPU/shader 视觉能力、复杂图形和 2.5D UI、跨桌面/移动/WebAssembly、运行时设计热更新、可写自定义
> shader、应用体积和性能可控。open-gpui 应在这些优势上进一步聚焦桌面生产力：代码/富文本、大列表/树/表、命令面板、dock、窗口级 overlay、多窗口多显示器、高 DPI、输入延迟和 AccessKit
> 语义，而不是单纯比拼炫目 shader。

#### `web_ecosystem_advantage`

> Web/Tauri/Electron 仍在组件数量、CSS/ARIA、浏览器辅助技术、设计 token 工具、Storybook/Chromatic、图表/地图/富文本 SDK、招聘语料和第三方插件上更成熟。Makepad 的 Web
> 目标是 wasm/WebGL 运行自身 runtime，不能直接继承 DOM 组件生态。open-gpui 应承认这一点，避免早期追完整 Web 组件面；更合理的是提供 Web 心智可迁移的组件 anatomy、token
> 名称、示例对照和必要 WebView/导出互操作，同时把 native 强项做到明显更好。

### 主题与设计系统

#### `theme_token_model`

> Makepad 的主题模型偏 Live DSL/shader 样式：颜色、布局、draw_bg/draw_text、shader 参数、状态动画和
> theme_desktop_dark/light、theme_mobile_dark/light 等模块可作为视觉入口；Live DSL 类似 CSS，可覆盖样式并支持运行时更新。它不像 DTCG 设计系统那样暴露独立语义 token
> schema、mode/fallback/state/variant registry。open-gpui 可借鉴“主题可热更新且可驱动 shader”的能力，但应把 semantic tokens、component
> tokens、state variants、density、motion、fallback 和 schema version 显式化。

#### `style_customization_boundary`

> Makepad 的样式边界是 framework/runtime 提供绘制和 shader 能力，内置 theme/widget 提供默认外观，Live/Splash DSL 负责布局、样式和节点配置，组件 prop/Live
> 字段暴露局部调参，用户 Rust/widget/shader 源码提供深度 escape hatch，Studio 提供设计时操作入口。这个边界适合 live design，但对 open-gpui 还需要更硬的生态边界：core
> behavior 与 a11y 不依赖主题，theme recipe 不改行为，component prop 只控制受支持变体，用户源码可替换 parts，app adapter 处理平台策略。

### 组件表面

#### `component_coverage`

> 覆盖度中高且偏应用平台：docs.rs 模块和 re-export 包括
> button、check_box、radio_button、slider、text_input、label、link_label、image、icon、markdown、html、video、view、adaptive_view、scroll_bar/scroll_bars、drop_down、modal、tooltip、popup_menu、popup_notification、portal_list、flat_list、dock、splitter、tab/tab_bar、stack_navigation、expandable_panel、color_picker、file_tree、window/multi_window、web_view、shader、debug/performance/designer
> 相关模块等。缺口是没有明确成熟的 headless Dialog/Menu/Select/Combobox/Table/Tree contract、form validation、a11y-first primitive 和统一 token
> schema。

#### `must_have_for_open_gpui`

> 必须借鉴的是 Rust-native GPU 自绘信心、Live/Splash 式可读 UI 描述、热更新反馈循环、Studio/inspector 对设计开发协作的价值、shader 与 theme 的深度连接、draw
> list/DrawStep 增量绘制、portal/cached/list 类性能组件、跨平台打包工具链和 AI 生成可检查 UI 的方向。open-gpui 首批必须补齐的不是 Makepad 全平台，而是 GPUI-native
> primitive
> contract：Dialog、Popover、Tooltip、Menu、Select/Combobox、Tabs、TextInput、Checkbox/Radio/Switch/Slider、ScrollArea、List/Table/Tree、Dock、Toast，以及
> focus、overlay geometry、AccessKit、theme token、gallery 和测试门禁。

#### `do_not_chase`

> 当前阶段不应追 Makepad 的完整 Studio 产品、AI IDE、独立 Live/Splash DSL 全量路线、多后端图形栈、移动/WebAssembly/Android/iOS 全平台、shader
> 创作工具、PDF/地图/语音/3D/XR/CEF 等长尾能力，也不应把所有样式问题都推给 shader。open-gpui 更应聚焦 GPUI 上层组件生态、行为/a11y/overlay/theme
> contract、桌面生产力控件和可验证分发闭环；Makepad 的创意工具路线适合作参考，不宜直接变成 open-gpui 的范围。

### 治理

#### `versioning_and_breakage`

> Makepad 2025 年发布 1.0，docs.rs 显示 `makepad-widgets` 1.0.0；当前官网又在强调 Makepad 2.0、AI Native Rust UIs 和
> Splash，说明路线仍在快速演进。Cargo 依赖可锁版本，但 Live/Splash DSL、内置 widget、shader、资源打包、Studio 和多平台工具链的 breaking change 面较大。open-gpui
> 若建设通用 UI 框架，应比 Makepad 更早定义稳定 public API、component contract version、theme schema version、migration guide、compatibility
> tests 和 deprecation 周期。

#### `maintenance_cost`

> 维护成本很高。Makepad 同时维护平台抽象、窗口/输入、GPU 后端、shader compiler、Live/Splash DSL、widget 库、Studio、AI
> workflow、examples、wasm/mobile/desktop 工具链、字体/文本/图像/vector/音视频/网络资源和打包文档。open-gpui 不应复制这个全栈负担；更现实的做法是只吸收高杠杆部分：可读 UI
> 描述、热更新/inspector、GPU-friendly theme、overlay/focus/a11y primitive、gallery/test harness 和少量高价值桌面组件。

#### `risks`

> 主要风险是把 Makepad 的 DSL/Studio/platform 路线误判为 open-gpui 的必选项，导致范围膨胀、学习成本升高、Rust/GPUI 原生 API 被第二语言割裂。第二个风险是 shader-first
> 视觉能力掩盖 a11y、键盘、状态机和组件 contract 的缺口。第三个风险是追逐移动、WebAssembly、3D、PDF、地图、语音、XR 等平台能力稀释桌面 UI 核心。第四个风险是没有显式 manifest/test gate
> 时，AI 生成和 Live 修改只能“看起来能跑”，难以证明行为、语义和性能仍正确。

#### `open_gpui_relevance`

> 建议为 reference-only + targeted trial。不要采用 Makepad 作为 open-gpui 架构母体，也不要直接引入 DSL-first 平台路线；应定向试验 Live/Splash 式可读 UI
> 描述、热更新/inspector、shader/theme 参数、draw step 增量渲染和 AI 生成验证闭环。直接设计含义是 open-gpui 应保持 Rust/GPUI-native API 为主，另加可选机器可读
> component manifest；优先建设 typed state machine、part anatomy、AccessKit mapping、overlay geometry、theme token、gallery
> manifest、visual/interaction/performance tests，让 AI 和开发者修改后有硬验证。

### 不确定字段（已跳过）

- `accessibility_model`
- `component_anatomy_model`
- `design_token_pipeline`
- `diagnostics_and_failure_quality`
- `docs_gallery_model`
- `interaction_state_machines`
- `machine_readable_contracts`
- `positioning_and_collision_model`
- `registry_viability`
- `testing_strategy`
- `third_party_ecosystem_path`

## <a id="tauri"></a>20. Tauri

- 结果文件：`Tauri.json`
- 调研类别：`webview_desktop_route`
- 纳入原因：
  Web 前端 + Rust backend 的主流轻量桌面路线；用于判断 open-gpui 应该避开哪些生态正面竞争，保留哪些互操作机会。
- 参考来源：
  - https://tauri.app/

### 定位

#### `positioning`

> Tauri 的生态定位是 WebView 桌面与移动应用框架、桌面 shell、权限化 IPC 层、插件平台、打包分发工具链，而不是原生 UI 组件库、headless primitive 或设计 token 系统。它让前端用
> HTML、CSS、JavaScript 或 WASM 构建界面，让 Rust、Swift、Kotlin 等承担系统能力和后端逻辑，并通过 TAO、WRY、命令、事件、权限、capabilities、CLI、bundler 和插件生态把
> Web 应用包装成系统应用。

#### `target_users`

> 主要服务已有 Web 前端能力、希望快速交付轻量桌面或移动应用的产品团队；熟悉 React、Vue、Svelte、Solid、Angular、Yew、Leptos 等前端栈的开发者；以及需要用 Rust
> 访问文件系统、窗口、托盘、通知、Shell、SQL、更新器等系统能力的应用团队。对 open-gpui 来说，它代表最成熟的“Web 前端 + Rust backend + 系统 WebView”竞争路线。

#### `primary_value_proposition`

> 核心价值是保留 Web UI 生态和浏览器开发体验，同时用系统 WebView 减少打包体积，用 Rust 后端与权限模型提升系统集成和安全边界。与 open-gpui 的匹配点是 Rust 桌面应用、系统集成、插件化、配置
> schema、CLI、测试和打包经验；不匹配点是 Tauri 的 UI 运行时本质仍是 DOM/WebView，open-gpui 应避免在普通 Web 组件、营销页面、后台表单和 npm 生态规模上正面竞争，而应在原生高密度桌面
> UI、文本、低延迟输入、复杂窗口/浮层和 AccessKit 上差异化。

### 分发与生态

#### `distribution_model`

> Tauri 采用 scaffold + package dependency + CLI + plugin marketplace-like directory + platform bundler 的混合分发方式。新项目可通过
> `create-tauri-app`、npm/yarn/pnpm/deno/bun 或 Cargo 入口创建；应用目录通常包含前端项目和 `src-tauri` Rust 子项目；核心依赖来自 Cargo crate、`@tauri-
> apps/api`、`@tauri-apps/cli`、Rust `tauri-cli`、TAO/WRY、插件 crate 与对应 JS API 包。官方插件覆盖 autostart、clipboard、dialog、fs、global
> shortcut、http、log、notification、opener、process、shell、sql、store、stronghold、updater、websocket、window state
> 等能力；分发端支持平台安装包、签名、store、GitHub Actions 和 updater。它不是源码复制型组件 registry，而是运行时依赖、配置文件、权限文件、插件包和打包产物的组合。

#### `source_ownership`

> 开发者拥有应用源码、前端源码、`src-tauri` Rust 源码、命令、插件接入、配置、权限、capabilities 和构建脚本；Tauri core、WRY、TAO、官方插件和 JS API 通常作为依赖消费，不是 copy-to-
> own。用户可以 fork 或 patch crate、插件、npm 包，但常规升级依赖 Cargo、npm、Tauri CLI、插件版本、前端构建工具和平台 SDK 的兼容。相比 shadcn 式源码所有权，Tauri
> 给应用层足够控制权，但 WebView、窗口运行时、权限实现和插件内部行为仍是依赖包边界，深度修改成本更接近框架 fork。

#### `rust_distribution_fit`

> Tauri 与 Rust 分发模型适配度高：核心和插件是 Cargo crate，CLI 有 Rust 和 JS 包装入口，项目有 `src-tauri/Cargo.toml`，配置通过
> `tauri.conf.json`、capabilities、permissions 和 build script 进入编译期，命令和宏由 Rust
> 类型系统、serde、`tauri::command`、`generate_handler`、`generate_context` 串联。对 open-gpui 的启发是：Rust-native UI 生态应优先拥抱 Cargo
> workspace、feature flags、SemVer、`cargo add`、`cargo generate` 或自定义 CLI、xtask、examples 和 schema 生成，而不是额外发明一套脱离 Cargo 的包管理。

### AI 时代设计

#### `copy_modify_verify_loop`

> Tauri 的闭环是 scaffold 或接入现有前端项目，编辑 Web UI 与 Rust 命令，运行 `tauri dev`，借助前端 HMR、浏览器 DevTools、Rust 编译、Cargo
> tests、前端单测、mockIPC、WebDriver、平台打包和 updater 验证。它不是复制组件源码再修改的模型。对 open-gpui 来说，可借鉴的是“一套 CLI
> 串起模板、运行、权限、测试、打包、发布”的工程闭环；组件复制或生成后仍应增加 cargo fmt、nextest、gallery 编译、截图 golden、AccessKit 快照、交互回放和性能 gate。

### API 与组合

#### `api_ergonomics`

> API 体验是双栈组合：前端照常使用 Web 框架和 DOM/CSS；前端调用 Rust 用 `invoke`、事件、channels 或 JS API 包；Rust 侧用
> `#[tauri::command]`、`Builder`、`invoke_handler`、`Manager`、`AppHandle`、`WebviewWindow`、plugin builder 和 serde
> 类型；系统能力通过官方插件暴露为 JS/TS API 和 Rust setup。优点是 Web 团队上手快、Rust 系统能力集中、权限边界显式；缺点是 API 跨语言、跨事件循环、跨序列化边界，复杂状态和大数据流需要额外设计。

#### `customization_model`

> Tauri 的定制分层是：界面由 Web 前端完全控制，结构和样式使用所选 Web 框架、CSS、Tailwind、design system 或 npm 组件；系统能力通过 Rust
> 命令、插件、capabilities、permissions、scope、sidecar、menu、tray、window/webview API 扩展；窗口和打包通过
> `tauri.conf.json`、平台配置、icons、bundle、signing、updater 控制。escape hatch 充足，可以直接写 Rust、调用系统 API、扩展插件或使用 WRY/TAO；但 UI 样式和交互并非
> Tauri 规范的一部分。

#### `component_anatomy_model`

> Tauri 不提供 Root/Trigger/Content/Item/Portal 等 UI 组件 anatomy。复杂 UI anatomy 完全来自所选 Web 组件库，例如 Radix、shadcn/ui、MUI、Ant
> Design 或自研 React/Vue/Svelte 组件。Tauri 自身的 anatomy
> 更像应用壳：App、Window、Webview、Menu、Tray、Command、Event、Plugin、Permission、Capability、Sidecar、Updater。对 open-gpui 的启发是 shell
> anatomy 与 UI component anatomy 应分开建模，不能用 Tauri 的 window/webview 插件结构替代原生组件 parts contract。

#### `state_ownership_model`

> Tauri 的状态所有权是 Web 前端状态 + Rust 应用状态 + 系统资源状态的混合模型。UI 状态通常由 React/Vue/Svelte 等前端框架拥有；Rust 后端状态通过
> `Manager::manage`、`State<T>`、Mutex、async Mutex、Arc 或应用自定义服务持有；跨边界通信依赖 commands、events、channels、store/sql/stronghold
> 插件或自定义协议。它没有统一 controlled/uncontrolled 组件状态规范；open-gpui 应把应用拥有状态、组件内部瞬态状态、Entity/runtime handle、可提升状态和可序列化配置显式区分。

### Headless 与行为

#### `headless_boundary`

> Tauri 的 headless 边界不是 UI 行为层，而是应用壳边界：Web UI 负责表现和大部分交互，Rust backend 负责系统能力和安全敏感逻辑，IPC
> 负责跨边界调用，capabilities/permissions/scope 负责最小权限，plugins 负责扩展系统 API。这个分层对桌面壳很清晰，但无法提供 menu/select/dialog/table/tree 等原生组件的
> renderer-neutral 行为、AccessKit metadata、positioning contract 或 style recipe。open-gpui 可学习其权限化边界，但 UI primitive 需要独立
> headless 设计。

### 渲染与性能

#### `rendering_model`

> Tauri 的渲染模型是系统 WebView/DOM/CSS 渲染：应用使用操作系统已有 WebView 承载 HTML、CSS、JavaScript 和 WASM，窗口由 TAO 管理，WebView 由 WRY 管理，Rust 后端通过
> IPC 与前端通信。它不是 native retained UI、immediate mode、自绘 GPU scene 或 GPUI Element/Entity 渲染模型。它的性能和语义边界继承各平台 WebView，而不是由
> Tauri 自己实现布局、绘制、文本和可访问性树。

#### `native_advantage`

> native GPUI 相对 Tauri/WebView 应明显胜出的场景包括高密度桌面生产力 UI、代码/富文本编辑、大表格、大树、大列表、低延迟键鼠输入、复杂 docking、多窗口多显示器、像素级 overlay、精细 DPI
> 坐标、长时间常驻内存、GPU scene 合成、原生菜单/快捷键深度集成和 AccessKit 语义控制。Tauri 的优势在轻量包装 Web 应用；open-gpui 的机会在 WebView 抽象成本高、DOM
> 性能或语义不可控、用户期待真正原生桌面质感的区域。

#### `web_ecosystem_advantage`

> Web/Tauri 生态天然更强在前端组件数量、CSS 布局和动画、DOM/ARIA、浏览器 DevTools、HMR、npm 包、图表、表单、管理后台、Markdown/HTML
> 内容、Storybook/Chromatic、设计系统工具、Web hiring、现有 Web 代码复用、跨平台静态页面和快速产品迭代。open-gpui 应主动避开“把 Web 应用包成桌面”的主战场，保留
> WebView/HTML/Markdown/前端资源嵌入与设计 token 互操作，把生态叙事集中在原生桌面强项。

### 主题与设计系统

#### `theme_token_model`

> Tauri 本身没有 UI theme token 模型。主题由 Web 前端生态决定，可以是 CSS 变量、Tailwind、design token pipeline、Material/MUI、shadcn/ui、Ant
> Design、自研主题或浏览器 prefers-color-scheme；Tauri 只影响窗口外壳、系统菜单、托盘、平台窗口效果和少量 WebView/window 配置。对 open-gpui 来说，Tauri 说明应用壳不应承担 UI
> token 语义；open-gpui 需要独立的 typed token schema、runtime theme、mode/state/variant、fallback 和平台适配。

#### `style_customization_boundary`

> Tauri 的样式边界很清楚：framework 负责应用壳、WebView、IPC、权限和打包；前端框架和用户源码负责 UI 样式、结构、交互与主题；插件负责系统能力；平台配置负责窗口、菜单、托盘、图标、签名、bundle
> 和更新。这个边界对 open-gpui 的启发是不要把系统 shell 能力和 UI component recipe 混在同一层；原生 UI 应把 core behavior、theme recipe、component
> prop、用户源码 override、app adapter、platform shell 分层。

### 组件表面

#### `component_coverage`

> Tauri 不提供通用 UI
> 组件覆盖。它覆盖的是应用壳和系统能力：窗口、WebView、多窗口、菜单、托盘、对话框、文件系统、剪贴板、通知、全局快捷键、进程、Shell、HTTP、SQL、Store、Stronghold、Updater、WebSocket、Window
> State、Deep Link、Autostart、移动端插件、sidecar、资源嵌入、打包和签名。Button、input、select、dialog、menu、table、tree、tabs、toast、chart 等 UI 组件来自
> Web 生态。

#### `must_have_for_open_gpui`

> open-gpui 必须补齐的不是 Tauri 的 WebView UI，而是可互操作的应用壳能力和工程工具：窗口/多窗口基础、菜单、托盘、快捷键、文件对话框、通知、deep link、更新器接口、打包/签名指南、权限化系统 API
> 思路、配置 schema、插件边界、CLI scaffolding、examples、测试和诊断。组件层面必须优先形成 Tauri 没有的原生优势：AccessKit contract、focus/keyboard、overlay
> positioning、Button/Input/Select/Menu/Dialog/Tooltip/Popover/Tabs/Table/Tree/Text、theme token、gallery 和验证门禁。

#### `do_not_chase`

> 当前阶段不应追 Tauri 的完整 WebView 桌面壳路线：不要复刻前端框架适配矩阵、SSR/SSG 配置文档、npm 插件生态、移动端 Swift/Kotlin 插件体系、所有平台打包商店指南、WebDriver/Tauri
> service 全套测试生态、updater/cloud 商业链路、WebView 安全模型和 Electron 替代叙事。open-gpui 也不应追 Web 组件数量、CSS 细节、DOM portal 生态和 Web
> 管理后台模板；这些是 Tauri 的天然主场。

### 治理

#### `versioning_and_breakage`

> Tauri 的版本治理由 core、CLI、JS API、Rust crates、插件、配置 schema、permissions/capabilities、前端工具和平台 SDK 共同构成。Tauri 2 把大量系统能力迁到插件，以稳定
> core 并让插件独立迭代；也提供从 v1 到 v2、beta 到 stable 的迁移文档和 breaking change 说明。风险在于应用跨 Cargo/npm/平台多条版本线，插件和权限命名变化会影响构建与运行。open-gpui
> 应采用 Cargo SemVer、schema version、experimental feature、migration guide、compatibility matrix、examples 编译矩阵和 public API
> drift 测试。

#### `maintenance_cost`

> Tauri 的维护成本很高但边界清晰：核心团队要维护 Rust core、WRY/TAO、runtime abstraction、IPC、安全模型、权限/capabilities、CLI、create-tauri-app、JS API、插件
> workspace、移动端 Swift/Kotlin 绑定、测试工具、打包签名、updater、文档和社区插件入口。open-gpui 不应复制这个全平台 shell
> 投入，而应选择性吸收配置、权限、插件、CLI、测试和分发经验，把主要维护预算投向 GPUI-native 渲染、组件 contract、AccessKit、theme、overlay、gallery 和高价值控件。

#### `risks`

> 主要风险是误把 Tauri 的成功归因于 Rust 后端而忽视其真正优势来自 Web 生态复用、系统 WebView 和成熟前端工具；第二是 open-gpui 若追普通 Web
> 应用场景，会在组件数量、CSS、DevTools、招聘和资料规模上输给 Tauri；第三是如果 open-gpui 引入 WebView 过深，原生性能优势和组件 contract 会被稀释；第四是 Tauri 式跨语言 IPC 也提示
> open-gpui 插件/系统能力要谨慎设计权限和数据流；第五是 AI 生成如果只有代码可编译而无视觉/a11y/交互 contract，结果仍不可验证。

#### `open_gpui_relevance`

> 建议 reference-only + interoperability trial。不要采用 Tauri 的 WebView 渲染路线作为 open-gpui 主路径，也不要追 Tauri/Electron 的 Web
> 应用包装生态；应参考其 Rust/Cargo + CLI + schema + permissions + plugin + bundler + updater + tests 的工程化能力，并保留与
> WebView、HTML、Markdown、现有 Web 前端资源的互操作。直接设计含义是 open-gpui 应把自己定位为 GPUI-native 原生 UI 框架：核心差异是 typed component
> contract、AccessKit-first、overlay geometry、theme token、gallery/test 同源、Rust-native performance，而不是又一个 Web 前端桌面壳。

### 不确定字段（已跳过）

- `accessibility_model`
- `ai_friendliness`
- `design_token_pipeline`
- `diagnostics_and_failure_quality`
- `docs_gallery_model`
- `interaction_state_machines`
- `machine_readable_contracts`
- `performance_model`
- `positioning_and_collision_model`
- `registry_viability`
- `testing_strategy`
- `third_party_ecosystem_path`

## <a id="electron"></a>21. Electron

- 结果文件：`Electron.json`
- 调研类别：`webview_desktop_route`
- 纳入原因：
  Web 桌面生态事实标准之一；应作为生态速度、开发者心智、插件/组件复用能力的强参照，也作为性能/体积反例。
- 参考来源：
  - https://www.electronjs.org/docs/latest/

### 定位

#### `positioning`

> Electron 的生态定位是 WebView/Chromium 桌面应用框架、desktop shell、Node.js + Chromium runtime、IPC 与系统 API 桥接层、打包发布工具链和 npm 生态入口，而不是原生
> UI 组件库、headless primitive 或设计 token pipeline。它把 HTML/CSS/JavaScript/WebAssembly 的 Web UI 与 Node.js、Chromium、V8、native
> addon、BrowserWindow、webContents、Menu、Tray、dialog、protocol、session、autoUpdater、crashReporter 等能力组合成跨平台桌面应用平台。

#### `target_users`

> 主要服务已有 Web 技术栈、需要快速交付 macOS/Windows/Linux 桌面产品的团队；熟悉 React、Vue、Svelte、Angular、TypeScript、Node.js、npm 和浏览器 DevTools
> 的开发者；需要复用 Web 组件、Web 内容、Web 构建链和原生系统能力的桌面产品团队。对 open-gpui 来说，它是 Web 桌面路线的事实标准参照，也是需要明确避开的体积、内存、进程复杂度和 DOM 性能对照组。

#### `primary_value_proposition`

> 核心价值是用最大规模的 Web 生态构建桌面 UI，同时通过捆绑 Chromium、Node.js 和原生 API 获得稳定一致的运行时、强开发者心智、npm 包复用、成熟调试工具和跨平台发布能力。与 open-gpui 的匹配点是桌面
> shell、窗口、多进程、IPC、安全边界、打包发布、测试和生态速度；不匹配点是 UI 渲染依赖 DOM/Chromium，体积和内存成本高，复杂高密度原生桌面 UI、低延迟文本/表格/树/浮层和 AccessKit
> 可控性不是它的天然强项。

### 分发与生态

#### `distribution_model`

> Electron 采用 npm package dependency + CLI/scaffold + boilerplate/template + Forge/electron-builder 打包工具 + ASAR + prebuilt
> binary + npm 第三方包生态的混合分发方式。核心依赖通常是 `electron` npm 包，项目通过 `package.json`、npm/yarn/pnpm 脚本、主进程入口、预加载脚本、渲染进程前端构建产物和 Forge
> 配置组织；Electron Forge 负责 `start`、`package`、`make`、makers、签名、公证和发布流程；应用可以手工基于 Electron 预构建二进制和 `app.asar` 打包。它没有官方 UI 组件
> registry，UI 组件来自 npm/Web 生态，系统能力通过 Electron core API、native Node modules、Forge 插件、社区包和应用自写主进程代码扩展。

#### `source_ownership`

> 开发者拥有应用源码、主进程、preload、渲染进程、前端组件、CSS、构建脚本、Forge 配置、打包配置和本地 native module glue code；Electron
> core、Chromium、Node.js、V8、Electron API、Forge、packager、builder 和大量 npm 包通常以依赖形式消费。Electron 是开源 MIT 许可，可以 fork 或
> patch，但常规项目不会复制 core 源码；深度修改运行时意味着承担 Chromium/Node/Electron 版本、平台二进制、代码签名和安全更新成本。相比 copy-to-own UI，Electron
> 的应用层自由度极高，但运行时行为和跨平台细节仍是大型依赖边界。

#### `rust_distribution_fit`

> Electron 与 Rust 分发模型的直接适配度较低：主路径是 npm、Node.js、JavaScript/TypeScript、Chromium 和 native Node addon；Rust 可以通过 napi-
> rs、Neon、WASM、sidecar 或本地服务接入，但 Cargo、feature flags、workspace、crate SemVer、`cargo add`、`cargo generate` 和 nextest 不是
> Electron 的一等分发面。对 open-gpui 的启发是负面的也很重要：不要让 Rust 原生 UI 被 npm/JS 构建链牵引；应保留 Electron 那种 `one command
> scaffold/run/package/test` 的体验，但底层用 Cargo workspace、feature gates、xtask、examples、benches、schema 和 crates.io 对齐 Rust 心智。

### AI 时代设计

#### `ai_friendliness`

> 较高。Electron 对 AI 友好的原因是资料极多、Web/Node/TypeScript 心智成熟、API 文档稳定、官方示例可直接在 Electron Fiddle 打开、npm 包和 Stack Overflow/GitHub
> 语料丰富、Playwright/WebDriverIO/Selenium 可以驱动真实应用。限制也很明显：AI 修改通常跨越主进程、preload、渲染进程、IPC channel、安全配置、CSP、打包配置、native module
> 和平台差异；代码能运行不代表安全、性能、焦点、a11y 和打包都正确。open-gpui 应学习其可检索性和示例密度，但用更强的 typed contract 降低 AI 猜测空间。

#### `copy_modify_verify_loop`

> Electron 的闭环是从模板、boilerplate 或现有 Web 应用开始，编辑 `main` 主进程、`preload` 安全桥、HTML/CSS/JS/TS 渲染进程和前端组件，运行 `electron` 或
> `electron-forge start`，用 Chromium DevTools、console、source map、Node inspect、Chrome
> tracing、WebDriverIO、Playwright、Selenium、unit tests、E2E、打包 `make`、签名和自动更新流程验证。它不是源码组件 registry 的复制修改模型。对 open-gpui
> 来说，关键启发是把 scaffold、运行、调试、打包、截图、交互回放、a11y 快照和性能门禁串成一个本地循环。

### API 与组合

#### `api_ergonomics`

> API 体验是 Web 前端 + Node/Electron 主进程的双栈模型。UI 层照常使用 DOM、CSS、Web Components 或 React/Vue/Svelte 等框架；桌面能力在主进程中通过
> `app`、`BrowserWindow`、`webContents`、`Menu`、`Tray`、`dialog`、`shell`、`session`、`protocol` 等模块调用；渲染进程通过 preload 和
> `contextBridge` 暴露受控 API，再用 `ipcRenderer`/`ipcMain`、`invoke`、`send`、`postMessage`、MessagePort 和自定义 channel 通信。优点是 Web
> 团队上手极快、生态样例多、TypeScript 可补类型；缺点是跨进程和跨安全域边界明显，状态、错误处理、权限和性能需要工程纪律。

#### `customization_model`

> Electron 的定制分层非常开放：UI 样式和结构由 Web 技术、CSS、前端框架、npm 组件、Canvas/WebGL/WebGPU 或自研设计系统决定；桌面外壳由
> BrowserWindow/BaseWindow/WebContentsView、窗口样式、透明窗口、菜单、托盘、快捷键、通知、协议、session、剪贴板、文件对话框和系统偏好 API 决定；安全边界由
> preload、contextIsolation、sandbox、CSP、permission handler、navigation/new-window 限制和 fuses 控制；构建发布由
> Forge/builder/packager、ASAR、签名、公证和 updater 控制。escape hatch 极强，可以写 C/C++/Objective-C/Rust native addon 或 sidecar，但框架本身不治理
> UI 组件结构和主题一致性。

#### `component_anatomy_model`

> Electron 不提供 Radix/Ark 式 Root/Trigger/Content/Item/Indicator/Portal 组件 anatomy。复杂 UI anatomy 完全来自 Web 组件库或应用自研代码，例如
> React 组件、DOM portal、Floating UI、MUI、Ant Design、shadcn/ui、Radix、ProseMirror、Monaco 等。Electron 自身的 anatomy
> 更像桌面应用壳：App、BrowserWindow/BaseWindow、WebContents/WebContentsView、preload、renderer、IPC
> channel、Menu/MenuItem、Tray、session、protocol、utilityProcess、native module。对 open-gpui 的启发是 shell anatomy 与 UI component
> anatomy 必须分开建模，不能让窗口/WebView 架构替代原生组件 parts contract。

#### `state_ownership_model`

> Electron 的状态所有权是多层混合：UI 状态通常由 React/Vue/Svelte/DOM 或前端状态库拥有；主进程拥有应用生命周期、窗口、菜单、托盘、session、协议、系统资源、后台服务和原生能力状态；preload
> 暴露受控桥接 API；跨进程状态通过 IPC、MessagePort、custom protocol、文件、数据库、store、native module 或外部服务同步。Electron 没有统一
> controlled/uncontrolled 组件状态规范，也不提供 renderer-neutral UI state machine。open-gpui 应显式区分应用拥有状态、组件内部瞬态状态、Entity/runtime
> handles、可提升状态、可序列化状态和跨线程/跨窗口状态。

### Headless 与行为

#### `headless_boundary`

> Electron 的 headless 边界主要是安全和进程边界，而不是 UI primitive 边界：主进程负责特权系统 API 和生命周期，渲染进程负责 DOM UI，preload/contextBridge 负责最小暴露
> API，sandbox/contextIsolation/CSP/permission handler 限制不可信内容，IPC/MessagePort 负责通信。这个分层对桌面 Web 应用非常清楚，但不提供
> Button/Menu/Select/Dialog/Table/Tree 等组件的 renderer-neutral 行为、焦点规则、AccessKit metadata、positioning contract 或 theme
> recipe。open-gpui 可借鉴其进程隔离和权限思路，组件层必须另建 headless contract。

### 渲染与性能

#### `rendering_model`

> Electron 的渲染模型是捆绑 Chromium 的 DOM/WebView 渲染 + 多进程架构：每个 BrowserWindow 或 Web embed 对应渲染进程，主进程管理生命周期、窗口和系统 API，Chromium 负责
> HTML/CSS layout、style、paint、compositing、GPU、文本、输入和 accessibility tree，Node.js/V8 提供脚本和系统扩展能力。它不是 native retained
> UI、immediate mode、自绘 GPU scene 或 GPUI Element/Entity 渲染模型。

#### `native_advantage`

> native GPUI 相对 Electron 应明显胜出的场景包括小体积和低常驻内存、高密度桌面生产力 UI、代码/富文本编辑、大表格、大树、大列表、低延迟输入、复杂 docking、多窗口多显示器几何、精准 overlay、直接 GPU
> scene、Rust 类型和内存模型、无 JS/DOM bundle 的启动路径、可控文本 shaping 和 AccessKit 语义树。Electron 的优势是把 Web 做到桌面，open-gpui 的机会是把真正原生桌面 UI
> 做到可组合、可验证、可扩展。

#### `web_ecosystem_advantage`

> Electron/Web 生态天然更强在 npm 包数量、前端组件库、CSS 布局动画、DOM/ARIA、浏览器
> DevTools、HMR、Storybook/Chromatic、Playwright/WebDriver、Monaco/ProseMirror、Markdown/HTML、图表、视频、Canvas/WebGL/WebGPU、Web
> hiring、AI 语料和现有 Web 应用复用。open-gpui 不应追“把 Web 应用包成桌面”的主战场，也不应追普通后台表单、营销页、Web 内容渲染和 npm 组件规模；应保留 HTML/Markdown/WebView/设计
> token 互操作，把差异化押在原生桌面强项。

### 主题与设计系统

#### `theme_token_model`

> Electron 本身没有 UI theme token 模型。主题由 Web 前端生态决定，常见形态是 CSS variables、Tailwind config、design token 工具、MUI/Ant/shadcn/Radix
> theme、自研 CSS、prefers-color-scheme 和应用状态；Electron 只提供 `nativeTheme` 读取/响应 Chromium 原生色彩主题，可设置 system/light/dark，并影响部分
> Electron/OS 渲染的菜单、DevTools、窗口 frame 和 CSS media query。对 open-gpui 来说，Electron 提示 shell 主题信号和组件 token schema
> 要分层：系统主题只能是输入，不是完整设计系统。

#### `style_customization_boundary`

> Electron 的样式边界很清楚但很松：Electron framework 负责窗口、WebContents、IPC、安全、系统 API 和打包；前端框架、CSS、Web 组件库和用户源码负责 UI
> 样式、结构、布局和交互；nativeTheme/systemPreferences 只提供系统主题和偏好信号；Forge/builder
> 负责图标、窗口资源、安装包、签名和发布。这个边界给开发者最大自由，但不会自动防止设计系统漂移。open-gpui 应保留清晰分层，同时比 Electron 更强约束 component prop、theme recipe、token
> path、用户 override、app adapter 和 platform shell 的责任边界。

### 组件表面

#### `component_coverage`

> Electron 不提供通用 UI 组件覆盖，但通过 Web 生态间接拥有极高覆盖度。Electron core
> 覆盖应用壳和系统能力：窗口、WebContents/WebContentsView、菜单、上下文菜单、托盘、对话框、通知、剪贴板、globalShortcut、protocol、session、net、desktopCapturer、screen、shell、safeStorage、crashReporter、autoUpdater、nativeImage、nativeTheme、utilityProcess、MessagePort、TouchBar、pushNotifications、inAppPurchase
> 等。Button、input、select、tabs、dialog、popover、table、tree、toast、chart、editor 等 UI 组件来自 DOM、浏览器和 npm 组件库。

#### `must_have_for_open_gpui`

> open-gpui 必须补齐的不是 Electron 的 WebView 渲染路线，而是它证明用户会期待的桌面工程闭环：窗口/多窗口、菜单、上下文菜单、托盘、快捷键、文件对话框、通知、剪贴板、deep
> link/protocol、更新器接口、crash/log、打包/签名指南、scaffold、dev server、examples、debugging、E2E 和发布文档。组件层面必须补齐 Electron
> 没有的一等原生能力：AccessKit contract、focus/keyboard、overlay
> geometry、Button/Input/Select/Menu/Dialog/Tooltip/Popover/Tabs/List/Table/Tree/Text、theme token、gallery 和可验证测试门禁。

#### `do_not_chase`

> 当前阶段不应追 Electron 的完整 Web 桌面生态：不要复刻 Chromium/WebView runtime、Node/npm 插件宇宙、前端框架矩阵、webpack/vite/parcel 配置文档、全部 Web
> 组件库、electron-builder/Forge 全量平台发布能力、复杂自动更新商业链路、Chrome 扩展/DevTools 生态、WebRTC/视频/DRM/浏览器级能力和 Web 应用包装叙事。open-gpui
> 也不应为了生态速度牺牲原生性能、二进制体积、内存、AccessKit 可控性和 Rust-native API 清晰度。

### 治理

#### `versioning_and_breakage`

> Electron 的版本治理由 Electron major/minor/patch、Chromium、Node.js、V8、npm dependency、native module ABI、Forge/builder、平台 SDK
> 和操作系统策略共同构成。官方 major 版本跟随每隔一个 Chromium major，大约 8 周一个 Electron major，支持最新三个 stable major；breaking API change 通常尽量至少保留两个
> major 的兼容窗口，并维护 breaking changes 文档和 release timeline。风险在于应用实际上捆绑浏览器和 Node，安全更新要求频繁升级；native modules、IPC、安全默认值、Chromium
> 行为和打包工具变化都可能带来迁移成本。open-gpui 应采用 Cargo SemVer、schema version、experimental feature、compat matrix、migration guide 和 API
> drift tests。

#### `maintenance_cost`

> Electron 的维护成本极高：核心团队要跟进 Chromium、Node.js、V8、安全漏洞、平台 API、窗口系统、GPU、IPC、sandbox、contextIsolation、fuses、native
> modules、prebuilt binaries、CI、文档、release cadence、Forge/工具生态和社区治理。应用团队的维护成本也高：需要管理 npm 依赖、安全审计、Chromium/Electron 升级、前端
> bundle、主/渲染进程边界、打包签名、公证、自动更新、平台差异和内存/启动性能。open-gpui 不应复制这个平台级负担，应选择性学习工程闭环，把维护预算投向 GPUI-native 渲染、组件
> contract、AccessKit、overlay、theme 和高价值控件。

#### `risks`

> 主要风险是被 Electron 的生态速度吸引后误入 WebView/DOM 主战场，导致 open-gpui 在 npm 组件规模、CSS、DevTools、招聘和资料量上与 Web 正面竞争；第二是若引入过多 WebView/JS
> 层，会稀释原生性能、体积、内存和 Rust API 优势；第三是 Electron 的安全经验说明跨特权边界、IPC、远程内容、依赖和 CSP 需要持续纪律，open-gpui 若做插件/脚本能力也会遇到类似风险；第四是 Electron
> 式自由组合容易造成组件碎片化、主题漂移和 AI 生成不可验证；第五是频繁 runtime 升级带来的维护压力不适合小核心团队照搬。

#### `open_gpui_relevance`

> 建议 reference-only + targeted interoperability trial。不要采用 Electron 的 Chromium/WebView 渲染路线作为 open-gpui 主路径，也不要追 npm/Web
> 组件生态规模；应把 Electron 作为生态速度、开发者心智、示例密度、调试、打包、IPC、安全和发布治理的强参照，同时把它作为体积、内存、多进程复杂度和 DOM 性能的反例。直接设计含义是 open-gpui 应明确定位为 Rust-
> native/GPUI-native 原生 UI 框架：Cargo-native 分发、typed component contract、AccessKit-first、overlay geometry、theme token
> schema、gallery/test/AI docs 同源、可选 WebView/HTML 互操作，而不是另一个 Web 桌面壳。

### 不确定字段（已跳过）

- `accessibility_model`
- `design_token_pipeline`
- `diagnostics_and_failure_quality`
- `docs_gallery_model`
- `interaction_state_machines`
- `machine_readable_contracts`
- `performance_model`
- `positioning_and_collision_model`
- `registry_viability`
- `testing_strategy`
- `third_party_ecosystem_path`

## <a id="tanstack-table-tanstack-virtual"></a>22. TanStack Table / TanStack Virtual

- 结果文件：`TanStack_Table_TanStack_Virtual.json`
- 调研类别：`headless_data_interaction_library`
- 纳入原因：
  复杂 headless 行为库的 API、state、extension、performance 边界参考，尤其适合 open-gpui table/tree/virtualized list。
- 参考来源：
  - https://tanstack.com/table/latest
  - https://tanstack.com/virtual/latest

### 定位

#### `positioning`

> TanStack Table 与 TanStack Virtual 属于复杂数据交互的 headless 行为库。Table 负责表格/数据网格的列、行、单元格、排序、过滤、分组、分页、选择、展开等状态与派生模型；Virtual
> 负责长列表、网格、masonry、聊天/日志等滚动虚拟化计算。它们不是视觉组件库，也不是主题系统，而是可被 React、Vue、Solid、Svelte、Qwik、Lit、Angular 等适配器承载的 renderer-neutral
> core。

#### `target_users`

> 主要服务需要构建复杂表格、大列表、树形数据、日志流、管理后台和设计系统数据组件的应用开发者、框架适配器作者与组件库维护者。对 open-gpui 来说，最相关用户是桌面产品团队、原生 UI 框架作者和需要可组合数据组件的库维护者。

#### `primary_value_proposition`

> 核心价值是把数据交互状态、派生模型和滚动性能从具体渲染层中剥离，使用户保留 100% 视觉和结构控制权。它与 open-gpui 的目标高度匹配：open-gpui 可以用原生渲染和滚动能力实现视觉层，同时复用 headless
> table/tree/virtualized list 的状态边界、扩展点和性能契约。

### 分发与生态

#### `distribution_model`

> TanStack 的分发方式以 npm package dependency 为主：Table 有 table-core 与各框架 adapter，Virtual 有 virtual-core 与各框架
> adapter。用户通过包管理器安装稳定 API，而不是复制源码到项目；文档和 examples 提供 recipes。它不是 shadcn 式 copy-to-own registry，也没有组件 marketplace。对 open-
> gpui 的启发是拆成 core crate、gpui adapter crate、examples/recipes，而不是把所有能力塞进单个视觉组件 crate。

#### `source_ownership`

> 用户通常不拥有库源码，只通过公开 API、类型、回调和扩展点定制行为；项目开源，必要时可 fork，但 fork/merge 成本高于 copy-to-own。Table 的 Custom Features、state
> hoisting、meta 和 row model 机制降低了 patch 核心库的需求；Virtual 的
> rangeExtractor、measureElement、scrollToFn、observeElementRect/Offset 等选项提供了替换底层行为的边界。

#### `registry_viability`

> TanStack 证明复杂行为库不一定需要源码 registry；对 native Rust UI，更可行的是 crates.io 上的 headless core + adapter crates，再配一个 machine-
> readable recipe/metadata registry。registry 的单位应是表格列模型、虚拟列表 recipe、性能测试场景、主题适配示例和 gallery case，而不是一堆不可验证的复制组件。

#### `rust_distribution_fit`

> 与 Rust 生态适配度高。Table core 可映射为 `open_gpui_table_core`，GPUI 绑定为 `open_gpui_table`；Virtual 可映射为 `open_gpui_virtual_core`
> 和滚动/测量 adapter。Cargo feature flags 可承载排序、过滤、分组、展开、选择、列尺寸、虚拟化等可选能力；SemVer 管 API 稳定性；`cargo add` 安装 crate；`cargo generate`
> 或 `xtask add-recipe` 生成示例和测试夹具。需要避免 TypeScript 式过度泛型在 Rust 中变成复杂 trait 地狱。

#### `third_party_ecosystem_path`

> 第三方生态可以走三条路径：一是基于核心 trait/extension hooks 提供行模型、列功能、树模型、排序器、过滤器；二是提供 recipes，例如无限滚动表格、聊天日志、可展开树表、masonry gallery；三是提供
> gallery examples 与性能基准。审核重点应放在 API compatibility、feature flag 边界、性能基准和 a11y contract，而不是仅审核截图外观。

### AI 时代设计

#### `ai_friendliness`

> 整体 AI 友好度较高：文档围绕核心对象、选项、状态片段和示例展开；TypeScript 类型让 AI 容易推断 `ColumnDef`、`Table`、`Row`、`VirtualizerOptions`、`VirtualItem`
> 等关系；headless 边界让 AI 可以分别修改数据行为和视觉渲染。短板是官方没有把 recipes、性能场景、a11y 义务做成统一 manifest，AI 生成后仍需要项目本地测试闭环。

#### `machine_readable_contracts`

> 主要机器可读契约来自 TypeScript 类型与稳定 API：Table 的列定义、状态片段、row model、feature options；Virtual 的
> count、getScrollElement、estimateSize、overscan、rangeExtractor、measureElement、scrollToIndex、takeSnapshot 等选项/方法。它不是
> JSON/YAML schema 或 typed registry。open-gpui 应在 Rust 类型之外额外生成 manifest，用于驱动 docs、gallery、scaffold、性能测试和 AI 检索。

#### `copy_modify_verify_loop`

> TanStack 的循环是安装库、复制示例结构、写自己的 markup/style、通过类型检查和运行时交互验证。它适合 package dependency，不适合无约束复制核心逻辑。open-gpui 若采用类似理念，应让 AI
> 或开发者复制 recipe，而不是复制底层 virtualizer/table core；修改后通过 contract tests、交互测试、截图/像素测试、滚动性能基准和 a11y tree 检查验证。

### API 与组合

#### `api_ergonomics`

> Table 的 API 以配置对象和核心实例为中心：传入 data、columns、getCoreRowModel 以及可选 row models，得到 table/header/row/cell
> 对象与方法；状态可局部或整体控制。Virtual 的 API 更像可替换运行时控制器：传入 count、getScrollElement、estimateSize，读取
> getVirtualItems/getTotalSize/scrollOffset，调用 scrollToIndex/scrollToOffset/measureElement/takeSnapshot。open-gpui
> 可借鉴这种“声明输入 + 可查询实例 + 小粒度回调”的模式。

#### `customization_model`

> 样式和结构完全由用户拥有；行为通过 options、callbacks、state、meta、row models、custom features 与 adapter 定制。Table 支持列定义、单元格/header
> 渲染模板、排序/过滤/分组/选择等功能组合；Virtual 支持
> overscan、rangeExtractor、scrollToFn、observeElementRect/Offset、measureElement、getItemKey、lanes、anchorTo、followOnAppend、缓存测量等。open-
> gpui 应把视觉 recipe、交互策略和性能策略拆成可替换层。

#### `component_anatomy_model`

> 它不是 Radix 式 root/trigger/content/item/indicator anatomy，而是数据对象
> anatomy：Table、ColumnDef、Column、HeaderGroup、Header、Row、Cell；Virtual 则是 Virtualizer、VirtualItem、Range、Rect、scrollElement。对
> GPUI Element/Entity 模型的启发是：复杂数据组件的 anatomy 可以先围绕数据与行为对象建模，再由 UI 层把这些对象投射成 element tree。

#### `state_ownership_model`

> Table 同时支持内部 state、initialState、局部 controlled state、onStateChange 回调和 fully controlled state；这适合把
> sorting、filters、pagination、rowSelection、columnVisibility 等状态提升到应用或服务端查询层。Virtual 内部维护
> scrollOffset、scrollRect、measurements、isScrolling、scrollDirection 等运行时状态，通过
> onChange、initialOffset、initialMeasurementsCache、takeSnapshot 与外部状态交换。open-gpui 应采用“核心状态可控，测量/滚动运行时有 handle”的双层模型。

### Headless 与行为

#### `headless_boundary`

> 边界非常清楚：Table 不渲染 DOM，不提供样式；Virtual 也不是组件，不渲染 markup。核心只负责数据模型、状态转换、测量和可见范围计算；adapter 只把核心接入具体框架生命周期；应用负责 semantic
> structure、视觉、theme、事件连接和平台无障碍。这是 open-gpui table/tree/virtual list 最值得直接借鉴的分层。

### 渲染与性能

#### `rendering_model`

> 渲染模型是 JavaScript/TypeScript headless core + framework adapter；核心不直接操作 DOM，最终渲染由 React/Vue/Solid/Svelte 等应用代码或 React
> Native 类平台完成。它不是 retained native，也不是 immediate mode 或 GPU scene 框架。

#### `performance_model`

> Table 的性能策略是按需启用功能和 row model pipeline：core row model、sorted/filtered/grouped/expanded/paginated 等派生阶段可组合，也支持服务端/manual
> 模式把大数据处理外移。Virtual 的性能策略更直接：只渲染可见范围，使用 estimateSize 初始化尺寸，动态 measureElement 校正，overscan 平衡空白和渲染成本，rangeExtractor 支持
> sticky/header/footer，getItemKey 稳定缓存，lanes 支持 masonry，anchorTo/followOnAppend
> 支持聊天和日志，initialMeasurementsCache/takeSnapshot 支持滚动恢复，shouldAdjustScrollPositionOnItemSizeChange 处理测量变化引发的跳动。

#### `native_advantage`

> open-gpui 的 native 优势应体现在大表格、大树、大日志流、代码/富文本行、复杂 selection/focus、列 resize、sticky header/column、GPU 文本与增量重绘。相比 DOM，GPUI
> 可以减少节点膨胀、减少 layout thrash，并把测量、滚动、文本 shaping、绘制缓存放在同一个原生管线中。TanStack 的 core/adapter 分离适合让 open-gpui 把性能优势集中在 adapter 和
> renderer，而不是行为 API。

#### `web_ecosystem_advantage`

> Web 生态优势在于已有 React/Vue adapter、npm 安装、HTML table 语义、ResizeObserver、成熟 examples、浏览器 DevTools 和大量社区 recipes。open-gpui
> 不应追逐完整 Web adapter 矩阵，也不应照搬 DOM/ARIA 细节；更现实的是借鉴数据/状态契约，并为 native 提供 AccessKit、GPU 渲染、Cargo 分发和平台级测试。

### 主题与设计系统

#### `style_customization_boundary`

> 样式边界非常激进：framework/core 不提供视觉，用户或组件库完全负责 markup、class、layout、theme token 和状态样式。对 open-gpui，建议 core crate
> 不包含颜色、间距、字体和边框；视觉 crate/recipe 根据 row/cell/header/virtual item 状态渲染样式；app adapter 保留最终覆盖权。

### 组件表面

#### `component_coverage`

> 覆盖面集中在 data display 与 performance primitive：复杂 table/data grid 行为、长列表/虚拟网格/聊天日志/masonry
> 等虚拟化。它不覆盖基础控件、form、overlay、navigation、feedback、application shell 或 rich editor。

#### `must_have_for_open_gpui`

> 必须补齐的是 headless table/tree/list core、统一 row/column/cell/header anatomy、可控状态片段、服务端/manual 模式、扩展 feature trait、虚拟化
> scroll/measure contract、sticky/header/selection/focus 与性能基准。没有这些，open-gpui 很难在桌面 IDE、数据库工具、日志工具和管理类应用中形成可复用数据组件能力。

#### `do_not_chase`

> 当前阶段不应追完整 TanStack Web 适配器矩阵、React hooks 形式、所有高级 table 功能的一次性复刻、npm 式插件生态、DOM ResizeObserver 细节、浏览器 smooth scroll 兼容技巧和
> purely web 的示例数量。open-gpui 应优先实现 native 必需的 tree/table/virtual list 垂直切片，再逐步扩展 grouping、pinning、masonry、聊天流等高级能力。

### 治理

#### `versioning_and_breakage`

> 治理模型是常规开源包 SemVer + 版本化文档 + migration guide。Table v8 的 API 面较大，破坏性变更通常通过 major version 消化；headless core 与 adapters
> 分离有利于控制破坏范围。open-gpui 应为 core contract 提供更强稳定性，为 recipe/gallery 允许更快迭代，并对 feature flags 与 extension traits 给出兼容策略。

#### `maintenance_cost`

> 完整实现成本高。Table 的状态片段、row model pipeline、列尺寸、pinning、grouping、selection、树展开和服务端/manual 模式会迅速扩大 API 面；Virtual
> 的动态测量、滚动锚定、masonry、聊天流、缓存恢复和跨平台滚动边界也很复杂。长期维护必须依赖纯 core 测试、性能基准、清晰 feature boundaries 和少量高质量 recipes，否则容易变成难以验证的数据网格巨兽。

#### `risks`

> 主要风险包括：把 Web API 形态硬搬到 Rust/native；过早追齐 TanStack 全功能导致 API 过重；headless 过度自由导致 a11y
> 和键盘交互不一致；虚拟化测量与原生滚动耦合不清造成抖动；扩展点太多导致第三方生态碎片化；AI 生成 recipe 缺少 contract tests；核心库与视觉组件职责混淆，稀释 GPUI 的原生性能优势。

#### `open_gpui_relevance`

> 建议为 trial：架构原则应 adopt，具体 TypeScript API 只做 reference-only。优先做一个 `table_core + virtualizer_core + gpui adapter`
> 的纵向原型，覆盖可排序表格、树形展开、虚拟滚动、selection、keyboard focus、sticky header 和 AccessKit 输出。若原型能通过大数据性能与 a11y 测试，再固化为 open-gpui
> 通用数据组件基础设施。直接含义是：open-gpui 的 table/tree/list 不应先做成视觉组件，而应先定义 renderer-neutral data/interaction/performance contract。

### 不确定字段（已跳过）

- `accessibility_model`
- `design_token_pipeline`
- `diagnostics_and_failure_quality`
- `docs_gallery_model`
- `interaction_state_machines`
- `positioning_and_collision_model`
- `testing_strategy`
- `theme_token_model`

## <a id="storybook-chromatic"></a>23. Storybook / Chromatic

- 结果文件：`Storybook_Chromatic.json`
- 调研类别：`docs_gallery_visual_testing`
- 纳入原因：
  组件 story、文档、视觉回归和交互测试事实标准；用于设计 native gallery 是否应承担 Storybook 类职责。
- 参考来源：
  - https://storybook.js.org/docs
  - https://www.chromatic.com/docs/

### 定位

#### `positioning`

> Storybook / Chromatic 的生态定位是 docs/gallery/visual testing 基础设施，而不是组件库、headless primitive 或渲染框架。Storybook 把组件的可枚举状态写成
> story，并围绕 story 派生文档、示例、交互测试、可访问性检查和本地/CI 预览；Chromatic 则把这些 story 发布到云端，提供视觉回归、交互回归、可访问性回归、UI Review 和团队评审工作流。

#### `target_users`

> 主要服务设计系统团队、前端/客户端组件库维护者、产品工程师、QA、设计师、文档维护者和需要可视化验证 UI 变更的 CI 团队。对 open-gpui 最有价值的对象是原生组件框架维护者、桌面产品团队、AI agent
> 和贡献者，因为他们需要一个可浏览、可测试、可复现的 native gallery。

#### `primary_value_proposition`

> 核心价值是把“组件在各种状态下应该长什么样、如何交互、如何被文档解释、如何在 CI 中防回归”收敛到 story 这一事实源。它与 open-gpui 的目标高度匹配，但匹配点不是复刻 Web UI，而是让 native GPUI
> 组件拥有等价的 story/gallery/contract/visual gate，从而支撑组件演进、AI 修改验证和跨平台回归。

### 分发与生态

#### `distribution_model`

> Storybook 通过 npm package、framework preset、addon、CLI 初始化和配置文件分发，常见入口是把 Storybook 安装进现有项目并维护本地 `.storybook` 配置、stories 文件和
> addons。Chromatic 通过 npm CLI、项目 token、CI 集成和云端项目分发能力，上传 build 后生成可分享的 Storybook、快照和评审记录。二者不是 copy-to-own 组件
> registry；它们分发的是工具链、配置、addon 生态和云服务能力。

#### `source_ownership`

> 使用者拥有自己的 stories、docs、测试和 Storybook 配置源码；Storybook/Chromatic 工具代码以依赖方式升级。好处是组件示例和验证用例贴近项目源码，patch story
> 成本低；代价是工具版本、framework adapter、addon 兼容性和 CI 配置需要持续维护。对 open-gpui 来说，gallery/story 源码应归项目所有，核心 runner、snapshot
> 引擎和报告格式由框架维护。

### AI 时代设计

#### `ai_friendliness`

> 很高。Storybook 的 story、args、controls、autodocs、play function、test runner、tags、文档页面和 AI manifest
> 把组件状态、输入、用法和验证路径结构化；Chromatic 把每个 story 的渲染结果、视觉 diff、交互失败、可访问性问题和评审状态持久化。对 AI agent 来说，这种“先枚举状态，再修改代码，再跑 story
> 级验证”的循环比直接改应用页面更可控。

#### `machine_readable_contracts`

> Storybook 已经具备强机器可读倾向：CSF stories、args/argTypes、controls、tags、docs/autodocs、index/manifest、test runner 和 addons
> 能描述组件状态与元数据；Chromatic 进一步消费 story 列表、build metadata、branch/commit、viewports、modes、browsers、visual baselines 和 test
> result。open-gpui 应把这些思想落实为 typed Rust story manifest，而不是只做人工浏览的 examples。

#### `copy_modify_verify_loop`

> 常规循环是：开发者为组件写 stories，使用 args/controls 暴露可变输入，用 play function 描述用户交互，用 docs/autodocs 生成说明，在本地 Storybook 中调试，然后在 CI 中由
> Chromatic 构建、截图、执行交互/可访问性检查并让团队批准 diff。open-gpui 可借鉴为：复制或生成 native 组件后，同步生成 story、fixtures、contract tests 和截图矩阵，再由 xtask
> 在本地与 CI 中验证。

### API 与组合

#### `api_ergonomics`

> Storybook 的 API 以 CSF 模块和 story 对象为核心：`Meta` 描述组件级元信息，单个 story 描述 args、parameters、decorators、tags、render 和 play；addons
> 通过参数和预览配置扩展行为。Chromatic 的 API 主要是 CLI、项目配置、CI 环境变量、branch/build metadata 和 per-story/per-parameter 控制。对 GPUI 更合适的形态是
> Rust builder 或宏定义 story metadata，同时保留可序列化 manifest。

#### `customization_model`

> Storybook 允许通过 decorators 包装 provider/theme/router/mock，parameters 控制 addon 行为，args/controls 改输入，docs blocks
> 改文档页面，globals 和 toolbars 切换全局模式，addons 扩展面板和能力。Chromatic 允许配置浏览器、视口、modes、diff 阈值、TurboSnap/依赖图优化、忽略区域和评审策略。open-gpui 的
> gallery 应支持 theme、density、locale、platform、DPI、字体、窗口尺寸和输入模式等 native 全局维度。

#### `component_anatomy_model`

> Storybook / Chromatic 本身不规定 root/trigger/content/item 等组件 anatomy；它们记录的是组件实例在某个 story 下的渲染和交互。它可以承载 anatomy 文档、parts
> 示例和状态矩阵，但 anatomy contract 应来自组件框架本身。对 open-gpui 来说，gallery 应消费 primitive 的 part/anatomy metadata，而不是把 anatomy 设计塞进
> story 工具。

### Headless 与行为

#### `headless_boundary`

> Storybook / Chromatic 的 headless 边界主要在工具层：story 是渲染框架无关程度较高的描述单元，但最终仍依赖具体 renderer/framework adapter
> 构建可视化预览。它不提供行为状态机、a11y metadata、layout/positioning 或 style/theme 分层方案；这些应由组件框架、headless primitive 和设计系统负责。open-gpui 应把
> story runner 设计成消费组件 contract 的工具层，而不是替代 primitive 设计。

#### `accessibility_model`

> Storybook 提供 Accessibility 测试 addon，可在组件 story 上运行基于 axe 的检查，并与测试/文档工作流结合；Chromatic 提供云端可访问性回归能力，能在 CI 和评审中暴露 story
> 级问题。对 open-gpui 不能直接复用 ARIA/axe，应改为 AccessKit 或平台辅助技术 contract：每个 story 附带 role、label、value、focus
> order、actions、relationships、keyboard path 和截图/树快照，失败时定位到具体组件与节点。

#### `positioning_and_collision_model`

> Storybook / Chromatic 不提供 overlay positioning、collision、flip、shift、safe polygon 或 dismiss 内核。它们能承载这些行为的 examples
> 和截图矩阵，例如不同窗口尺寸、滚动容器、边界碰撞和焦点返回场景。open-gpui 的 overlay 算法应由独立 primitive 实现，gallery 负责把边界条件枚举成可视化和交互回归用例。

#### `interaction_state_machines`

> Storybook 的 play function 和 test runner 可以表达 story 级交互脚本，例如点击、输入、键盘导航和断言结果；Chromatic
> 可在云端把交互过程纳入回归检查。它不是显式有限状态机系统，但很适合作为状态机 contract 的外层验收：每个 machine transition 或关键用户路径都映射为 story + play/test。open-gpui 应把
> primitive 状态机和 gallery interaction script 分开，让脚本验证而不是定义行为。

### 渲染与性能

#### `rendering_model`

> Storybook 的主流落地是 Web DOM 预览环境，依赖 React/Vue/Angular/Web Components 等 renderer 适配。Chromatic 构建并托管静态
> Storybook，在浏览器中截图和执行检查。对 open-gpui 需要替换为 native window/GPUI scene 渲染、离屏或虚拟显示截图、平台字体和 GPU 管线控制。

#### `native_advantage`

> native GPUI 的优势在 Storybook/Chromatic 不擅长的层面：真实桌面输入延迟、窗口管理、多显示器/DPI、字体与文本布局、GPU 场景、原生滚动、大树/表格和 AccessKit 节点。native
> gallery 若做得好，还能检查 Web 截图工具难覆盖的窗口焦点、系统主题、平台快捷键和屏幕阅读器树。

#### `web_ecosystem_advantage`

> Web 生态的优势非常明显：Storybook/Chromatic 已有成熟的浏览器预览、addons、文档生成、交互测试、视觉 diff、CI 集成、团队评审、云端托管和大量设计系统实践。open-gpui
> 不应短期追完整云平台和插件市场，而应先做本地 deterministic gallery、manifest、截图回归和基础报告；必要时与现有 Web 报告格式或静态站点互操作。

### 主题与设计系统

#### `theme_token_model`

> Storybook 可通过 decorators、globals、toolbars、parameters 和 docs 展示不同 theme/mode，但它不定义设计 token schema。Chromatic 的 modes
> 和快照矩阵可用于测试 light/dark、品牌主题、viewport、locale 等组合。open-gpui 应让 theme token pipeline 独立存在，gallery 只读取主题 manifest 并生成跨主题
> story 矩阵。

#### `design_token_pipeline`

> Storybook / Chromatic 不是 DTCG、Style Dictionary 或 Tailwind-like token 编译管线；它们是 token 输出结果的展示与回归验证层。最佳用法是 token pipeline
> 生成主题文件后，由 stories 覆盖不同 token mode，Chromatic 或 native visual gate 捕获 token drift。open-gpui 应避免把 token 编译器塞进 gallery，但应让
> gallery 能读取 token metadata 并显示/验证 token 使用结果。

#### `style_customization_boundary`

> 样式仍由组件、设计系统、theme provider 或应用源码负责；Storybook 负责提供展示容器、全局切换和文档说明，Chromatic 负责记录视觉结果。open-gpui 应保持这个边界：framework primitive
> 输出状态和 parts，theme recipe 生成视觉，gallery 负责组合 theme/args/story 并验证，不能让 gallery 成为样式运行时依赖。

### 组件表面

#### `component_coverage`

> Storybook / Chromatic 不提供业务组件覆盖；它们可以展示任意基础控件、form、overlay、navigation、data display、feedback、application shell 和 rich
> components。覆盖度取决于项目写了多少 stories。对 open-gpui 的启示是：组件覆盖不应只按 crate 导出清单衡量，还应按 story
> 覆盖矩阵衡量，包括默认、禁用、错误、加载、空状态、长文本、键盘焦点、主题和不同尺寸。

#### `must_have_for_open_gpui`

> 必须补齐，但应作为工具链能力而不是核心渲染 API。open-gpui 通用 UI 框架至少需要本地 gallery、story manifest、可交互 examples、文档派生、截图基线、交互脚本、AccessKit
> 快照、主题/尺寸矩阵、CI gate 和失败报告。没有这些，AI 生成组件和无畏重构都缺少可验证闭环。

#### `do_not_chase`

> 当前阶段不应追完整 Storybook 插件市场、Web iframe 预览架构、MDX 文档系统、Chromatic 云端 SaaS、浏览器矩阵、npm addon 兼容、React/Vue 框架适配和复杂协作评审产品。open-gpui
> 应追最小但硬核的 native story runner、deterministic screenshot、manifest、交互测试和报告格式。

### 文档测试工具

#### `docs_gallery_model`

> 这是最值得采用的部分。Storybook 把 story 作为组件文档、示例、controls、autodocs、play function、测试和 AI manifest 的中心；Chromatic 再把 story
> 转为可发布、可比较、可评审的构建产物。open-gpui 应建立同构模型：每个组件的 story 同时驱动本地 gallery、静态文档、AI examples、截图回归、交互测试、AccessKit 树检查和性能 smoke
> case，避免 docs、examples 和 tests 三套样例漂移。

#### `testing_strategy`

> Storybook 覆盖的测试面包括 story 级渲染、play function 交互测试、test runner、可访问性检查、文档/示例验证和与主流测试工具集成；Chromatic 强项是视觉回归、交互测试、可访问性回归、UI
> Review、CI 状态和受影响 story 优化。open-gpui 应采用分层策略：Rust unit/contract tests 验证 primitive，story runner 做交互和可访问性树，screenshot gate
> 做视觉回归，performance smoke 覆盖大列表/文本/overlay，manifest drift gate 保证文档与 API 对齐。

#### `diagnostics_and_failure_quality`

> Chromatic 的价值在于把失败定位到具体 story、快照、分支、提交和视觉 diff，并提供评审批准/拒绝流程；Storybook 本地能通过 controls、docs、play/test 把失败缩小到组件状态。对 AI
> 自动修复，open-gpui 还应进一步输出结构化 diagnostics：story id、args、theme、viewport、platform、失败节点、AccessKit 路径、截图 diff 区域、交互步骤、日志和建议修复层级。

### 治理

#### `versioning_and_breakage`

> Storybook/Chromatic 自身遵循工具依赖版本升级和迁移文档；项目 story 是源码资产，组件 API 变化会直接导致 stories、args、docs 和视觉基线失效。这个失效是成本，也是优点：它把 breaking
> change 显性化。open-gpui 应把 gallery manifest 作为 API 稳定性守门：组件 public props、part names、状态枚举、theme token 和 AccessKit contract
> 变化时必须触发 story/基线更新和迁移说明。

#### `maintenance_cost`

> 维护成本中高。团队需要为每个组件持续编写和更新 stories、fixtures、play scripts、a11y expectations、截图基线、主题矩阵和 CI
> 配置；工具本身还要处理跨平台截图稳定性、字体、DPI、异步动画、测试耗时和报告可读性。收益是它把文档、示例、设计评审和回归测试合并，长期能显著降低组件库演进风险。

#### `risks`

> 主要风险是把 gallery 做成展示橱窗而非验证系统，导致示例好看但 contract 不完整。第二个风险是 native 截图不稳定：字体、抗锯齿、GPU、DPI、平台主题和动画会制造噪声。第三个风险是过度复刻
> Storybook/Chromatic 的 Web/SaaS 形态，拖慢 open-gpui 核心 primitive 建设。第四个风险是 story 数量膨胀但缺少 manifest 约束，AI 生成和第三方贡献会变成不可维护样例堆。

#### `open_gpui_relevance`

> 建议 adopt 工具链思想、trial native gallery runner、defer 云端协作平台。直接设计含义是：open-gpui 的通用 UI 框架不应让 native gallery 承担组件 runtime
> 职责，但必须让它承担 Storybook 类职责的一部分，即 story/catalog 单一事实源、文档派生、交互与可访问性验证、截图回归和 CI 报告。Chromatic 的可借鉴点是 build-based visual
> gate、受影响 story 选择、diff 审核和失败质量，而不是云产品本身。

### 不确定字段（已跳过）

- `performance_model`
- `registry_viability`
- `rust_distribution_fit`
- `state_ownership_model`
- `third_party_ecosystem_path`

## <a id="design-tokens-community-group-style-dictionary"></a>24. Design Tokens Community Group / Style Dictionary

- 结果文件：`Design_Tokens_Community_Group_Style_Dictionary.json`
- 调研类别：`design_token_pipeline`
- 纳入原因：
  跨平台 token format、transform、theme artifact 与 schema 管线参考；与 open-gpui theme schema 强相关。
- 参考来源：
  - https://tr.designtokens.org/format/
  - https://styledictionary.com/

### 定位

#### `positioning`

> Design Tokens Community Group（DTCG）定位是跨工具、跨平台的设计 token 交换格式规范；Style Dictionary 定位是把设计 token
> 解析、合并、转换、格式化并输出到不同平台的构建管线。它们不是组件库、headless primitive 或渲染框架，而是设计系统的 schema、transform、artifact 生成基础设施。

#### `target_users`

> 主要服务设计系统作者、框架作者、前端与移动端平台团队、设计工具集成方、文档工具作者，以及需要把同一套主题 token 输出到 CSS、iOS、Android、Flutter、Compose、JavaScript 或自定义运行时的团队。对
> open-gpui 来说，核心用户是主题 schema 维护者、组件库维护者和希望生成可验证主题文件的 AI agent。

#### `primary_value_proposition`

> 核心价值是把设计决策从运行时代码中抽离为可交换、可校验、可转换的 token 源文件，再通过平台管线生成具体产物。它与 open-gpui 的匹配度很高：open-gpui 需要一个稳定的 theme
> schema、默认主题、用户主题覆盖、组件 recipe 和文档/gallery 之间的单一事实源，而 DTCG 与 Style Dictionary 刚好提供了格式与管线参考。

### 分发与生态

#### `distribution_model`

> DTCG 以公开规范分发，定义 JSON 文件结构、$value、$type、$description、$extensions、$deprecated、group、reference、composite token 等格式语义；Style
> Dictionary 以 npm CLI 与 npm module 分发，是构建设计 token artifact 的开发期工具。典型流程是 source/include/tokens 收集 token，深度合并为 dictionary，经
> preprocessors、expand、transforms、reference resolution、filters、formats、actions 输出到每个平台。它不是运行时依赖，也不是 copy-to-own 组件
> registry。

#### `source_ownership`

> token 源文件通常由项目或设计系统团队拥有，生成产物可以提交或在构建时生成；Style Dictionary 作为工具依赖存在，用户不需要拥有其内部源码，但可以注册自定义
> parser、preprocessor、transform、format、filter、action 和 file header。对 open-gpui 来说，应让应用和主题包拥有 token 源码，open-gpui 只提供
> schema、校验器、默认 transform 和生成器，避免把主题决策锁死在框架内部。

#### `third_party_ecosystem_path`

> 第三方生态最自然的入口是主题包、token preset、组件 recipe token、品牌覆盖层、文档示例和校验规则插件。open-gpui 可以要求第三方提交 DTCG 源 token、open-gpui 扩展字段、生成后的 GPUI
> theme artifact、schema 版本范围、视觉快照、对比度报告和变更说明；贡献路径可以是 Cargo crate、repo-local theme 目录、gallery 示例或未来 registry 条目。

### AI 时代设计

#### `ai_friendliness`

> 很高。DTCG 的 JSON 结构、明确的 $value/$type、引用语法、类型继承、$extensions 和 composite token 让 AI 容易检索、编辑和解释；Style Dictionary 的配置式
> pipeline、platform、transform、format、filter 和 dictionary metadata 也适合 AI 生成与检查。对 open-gpui，最关键是把 token
> schema、生成规则、组件消费边界和错误诊断做成机器可读 contract，让 AI 能从主题意图到 artifact 再到 visual/a11y 测试形成闭环。

#### `machine_readable_contracts`

> DTCG 本身就是机器可读格式契约：token 是含 $value 的对象，$type 可在 token 或 group 层声明并继承，引用支持花括号路径和 JSON Pointer，$extensions
> 允许厂商扩展且未知扩展应被保留。Style Dictionary
> 提供配置契约：source/include/tokens、usesDtcg、expand、platforms、transforms、transformGroup、files、filter、options.outputReferences、actions
> 等都可驱动产物生成。open-gpui 应在此基础上补 Rust 类型、JSON Schema、schema drift gate、artifact manifest 和组件 token usage manifest。

#### `copy_modify_verify_loop`

> 理想循环是复制或生成一套 token 源文件，开发者或 AI 修改语义 token、模式、状态和组件 recipe，再运行 schema 校验、引用解析、transform、artifact 生成、对比度/a11y 检查、visual
> snapshot 和主题 diff。Style Dictionary 已覆盖解析、合并、转换和输出；open-gpui 需要补 native 侧验证：确保生成 token 能被 GPUI theme runtime 读取，组件
> gallery 无缺失 token，暗色/亮色/高对比模式没有 schema 漂移。

### API 与组合

#### `api_ergonomics`

> API 形态偏构建配置而非 UI 调用。DTCG 的作者体验是编辑 JSON token 树并使用 $type/$value/$description/$extensions 表达语义；Style Dictionary
> 的体验是声明平台和文件输出，并通过 transforms、transformGroup、formats 和 actions 扩展。迁移到 open-gpui 时，运行时 API 不应暴露 Style Dictionary
> 风格细节，而应提供强类型 Theme、TokenKey、ColorRole、SpacingRole、TypographyRole、ComponentToken 和 ThemeResolver。

#### `customization_model`

> 定制主要发生在四层：token 源层改语义值和引用，DTCG 扩展层通过 $extensions 携带 open-gpui 特有 metadata，管线层通过 transform/format/filter/action
> 控制输出，运行时层通过 theme override、mode、state、variant 和 fallback 解析实际值。open-gpui 应允许用户替换主题文件、追加品牌覆盖、定义组件级 token、保留未知扩展，并提供清晰的
> escape hatch 让高级团队自定义 transform。

#### `component_anatomy_model`

> DTCG/Style Dictionary 不提供 root/trigger/content/item 等组件 anatomy。它们能表达的是组件 token 命名空间和 token 使用契约，例如
> button.background.default、button.background.hover、popover.surface.shadow、menu.item.padding、focus.ring.color。open-gpui
> 不应把 token pipeline 当作 anatomy 系统，而应让 anatomy/part schema 消费 token：每个 part 声明所需 token、状态 token 和 fallback。

#### `state_ownership_model`

> token pipeline 不管理 UI 状态，但可以表达状态相关的设计值，例如 default、hover、active、focus、disabled、selected、error、success、dark、high_contrast
> 等。open-gpui 的状态仍应由组件或应用拥有，theme resolver 只根据显式 state/mode/variant/context 解析 token，不应让 token 文件隐式决定组件状态机。

### Headless 与行为

#### `headless_boundary`

> 边界非常清楚：DTCG 定义设计值格式；Style Dictionary 负责构建期转换；它们不处理交互行为、焦点、布局、渲染或应用状态。open-gpui 应沿用这种分层：theme token schema 不依赖
> Element/Entity；组件行为不依赖生成器；渲染层只消费已解析的 typed token；样式 recipe 连接组件 part/state 与 token key。

#### `positioning_and_collision_model`

> DTCG/Style Dictionary 不覆盖 overlay 定位、collision、flip、shift、safe polygon、dismiss 或 focus return。它们最多提供
> popover/menu/tooltip 的尺寸、阴影、圆角、间距、动画和状态 token。open-gpui 应把 positioning 交给 overlay primitive，把 token pipeline 限定为 visual
> recipe 输入，避免把几何行为编码进主题 token。

#### `interaction_state_machines`

> 没有显式 finite state machine。token 管线可以列出状态枚举和状态 token，但不应承担 menu/select/dialog/tabs 等交互转换逻辑。open-gpui 可以让组件 contract 声明允许的
> state token key，并用测试确保状态机输出的 state 与主题 schema 可对齐。

### 渲染与性能

#### `rendering_model`

> DTCG 是 JSON 文件格式，Style Dictionary 是构建期 token 处理工具；二者没有 DOM、native retained、immediate mode、自绘或 GPU scene 渲染模型。

#### `performance_model`

> 性能关注点不在运行时渲染，而在构建期规模和产物质量：大量 token 文件的查找、解析、深度合并、引用解析、transitive transform、composite expand、多 platform 输出和文件写入。对 open-
> gpui 的性能策略是将复杂解析留在构建期，把运行时 artifact 做成紧凑 typed table 或 interned token map，避免每帧解析 JSON 或字符串引用；同时用 schema hash 和 artifact
> hash 做缓存与漂移检测。

#### `native_advantage`

> native GPUI 的优势不在 token 管线本身，而在消费产物后的运行时：可用紧凑 Rust 类型、arena/index、预解析颜色与尺寸、DPI 感知长度、字体度量缓存和组件级 token 访问表，减少 Web CSS
> cascade 与运行时变量解析成本。复杂桌面主题切换、高密度表格、编辑器、命令面板和多窗口场景可以从预编译 theme artifact 中受益。

#### `web_ecosystem_advantage`

> Web 生态在 token 工具、Style Dictionary 社区、CSS variables、Tailwind、Figma/Tokens Studio、主题包发布、设计系统文档和跨端输出上明显更成熟。open-gpui
> 不应早期自造完整 token 工具生态，而应兼容 DTCG 思路、可导入 Style Dictionary 产物或复用其输出概念，同时把 native 专有的 artifact、校验和 runtime resolver 做深。

### 主题与设计系统

#### `theme_token_model`

> 最值得采纳的模型是分层 token：基础 token 表达原始色板、尺寸、字体、阴影、时长；语义 token 表达 foreground/background/border/accent/danger/focus 等角色；组件 token
> 表达 Button、Input、Menu、Tooltip 等 part/state/variant；模式 token 表达 light/dark/high_contrast/reduced_motion。DTCG 的
> $type、group、reference、composite token 和 $extensions 可作为源格式，open-gpui artifact 应补 fallback、mode resolution、state
> resolution、variant resolution、deprecated token 和 unknown extension preservation。

#### `design_token_pipeline`

> 应重点参考。DTCG 提供跨工具格式，Style Dictionary 提供管线结构：source/include/tokens 输入、深度合并、preprocessor、composite
> expand、transform/transformGroup、reference resolution、filter、format、action 输出。open-gpui 可以设计等价的 Rust 原生管线：读取 DTCG 或 open-
> gpui theme source，校验 schema，解析引用和类型继承，展开 composite token，生成 Rust/JSON/RON/theme binary/gallery manifest，并对 schema
> drift、缺失 token、循环引用、未知类型和 deprecated token 失败给出结构化诊断。需要注意 Style Dictionary 文档说明其对 DTCG v4 起有一等支持，但最新 2025.10 格式尚未完全支持，因此
> open-gpui 不能把现有工具支持视为完全覆盖。

#### `style_customization_boundary`

> 样式边界应分为：framework 定义 token 类型、fallback 与 resolver；theme source 定义品牌和模式值；component recipe 定义 part/state/variant 如何消费
> token；component prop 只选择少量语义 variant/size/intent；用户源码可覆盖 recipe 或提供 app-level theme；构建管线负责把源 token 变成运行时
> artifact。不要让组件实现直接读取任意字符串 token，也不要让 token 文件决定交互行为。

### 组件表面

#### `component_coverage`

DTCG/Style Dictionary 不覆盖具体 UI 组件。它们覆盖的是组件视觉表面背后的主题能力：颜色、尺寸、字体、字号、行高、边框、阴影、渐变、过渡、间距、motion、状态值和平台产物。

#### `must_have_for_open_gpui`

> 必须补齐。一个通用 open-gpui UI 框架如果没有正式 theme token schema 和生成/校验管线，组件库会很快出现硬编码颜色、重复尺寸、暗色模式漂移、第三方主题不可验证和 AI
> 修改不可控的问题。建议优先实现最小闭环：DTCG-like 源格式、Rust typed theme artifact、默认 light/dark 主题、schema 校验、引用解析、组件 token usage
> manifest、gallery 快照和缺失 token 诊断。

#### `do_not_chase`

> 当前阶段不应追完整复刻 Style Dictionary 的所有平台输出、npm 插件生态、Figma 插件、Sketch 文件、Android/iOS/Flutter 全量产物、复杂 marketing design-system
> workflow 或任意用户脚本执行。open-gpui 应先做 native GPUI 所需的 schema、resolver、artifact 和测试门禁；跨平台输出可保留为导入/导出兼容层。

### 文档测试工具

#### `testing_strategy`

> 应采用多层测试：JSON/schema 测试覆盖必填字段、类型、命名、$extensions 保留和 deprecated 策略；引用测试覆盖花括号引用、JSON Pointer、循环引用、跨文件合并和 type 继承；transform
> 测试覆盖单位、颜色、字体、composite expand 与 transitive transform；artifact 测试覆盖 Rust 类型生成、hash、兼容版本；visual/a11y 测试覆盖组件 gallery
> 的亮暗模式、状态、对比度和 reduced motion；performance 测试覆盖大 token 集构建时间与运行时解析成本。

#### `diagnostics_and_failure_quality`

> DTCG 和 Style Dictionary 的结构天然适合高质量失败信息：错误可以定位到 token
> path、filePath、source/include、$type、$value、引用目标、platform、transform、format、filter 和输出文件。open-gpui 应把这进一步产品化：失败时输出具体 token
> key、组件 part、mode/state/variant、引用链、循环路径、期望类型、实际类型、候选修复和 schema 版本，让 AI 可以自动修改 token 文件而不是猜测样式代码。

### 治理

#### `versioning_and_breakage`

> 治理风险较高但可控。DTCG 当前页面是 2025.10 format module，且明确不是 W3C 标准；Style Dictionary 对 DTCG 从 v4 起一等支持，但文档也说明最新 2025.10
> 格式尚未完全支持。open-gpui 应把外部 DTCG 版本、open-gpui theme schema 版本和 artifact 版本分开：源格式可跟随 DTCG 子集，运行时 schema 用 SemVer
> 严格治理，breaking change 必须提供 migration guide、deprecated token 支持、compat transform 和 schema drift gate。

#### `maintenance_cost`

> 实现成本中高，长期收益高。最小可行成本包括 schema、parser、resolver、validator、生成器、默认主题、组件 token usage manifest 和测试；完整追 Style Dictionary 则需要大量平台
> format、插件 API、CLI、文档和生态维护。open-gpui 应采用分阶段策略：先做 native 必需管线和 DTCG 子集兼容，再根据真实主题和第三方需求扩展 transform/format 插件点。

#### `risks`

> 主要风险是过度工程化：在组件库尚未稳定前设计过大的 token 标准会拖慢交付。第二个风险是把 Web/CSS 变量思路机械搬到 native，导致运行时字符串解析、级联规则和性能优势被稀释。第三个风险是 DTCG
> 标准仍在演进，外部工具支持不完全，过早承诺全兼容会增加迁移负担。第四个风险是只生成 token artifact 而没有组件 usage manifest 和 visual/a11y gate，AI 仍可能生成不可验证的主题。

#### `open_gpui_relevance`

> 建议采纳（adopt）其核心思想，并试点（trial）Rust 原生实现。直接设计含义是：open-gpui 应定义一个 DTCG-like theme source schema，保留 $extensions 作为 open-gpui
> metadata 通道；实现独立 theme build/validate xtask；生成 typed runtime artifact 与 gallery manifest；要求组件声明 token usage；把 schema
> drift、引用错误、缺失 token、deprecated token、对比度和视觉快照纳入门禁。Style Dictionary 本身作为参考和可选互操作工具，不应成为 open-gpui 运行时依赖。

### 不确定字段（已跳过）

- `accessibility_model`
- `docs_gallery_model`
- `registry_viability`
- `rust_distribution_fit`

## <a id="accesskit"></a>25. AccessKit

- 结果文件：`AccessKit.json`
- 调研类别：`native_accessibility_infrastructure`
- 纳入原因：
  Rust/native accessibility 关键基础设施；open-gpui 的 a11y contract 应与 AccessKit 能力、限制和平台映射一起设计。
- 参考来源：
  - https://accesskit.dev/

### 定位

#### `positioning`

> AccessKit 的生态定位是 Rust/native UI 的 accessibility infrastructure：它提供跨平台可访问性数据 schema、平台 adapter、winit 集成、consumer crate
> 和语言绑定，让自绘 UI toolkit 只实现一次可访问性树，再映射到 Windows UI Automation、macOS NSAccessibility、Unix AT-SPI、Android、iOS 等平台
> API。它不是组件库、headless primitive、视觉框架、theme/token pipeline 或应用 shell，而是 open-gpui 这类原生自绘框架必须认真对齐的 a11y 中间层。

#### `target_users`

> 主要服务 UI toolkit/framework 作者、自绘 native 应用框架维护者、winit/glazier/egui/vizia 这类 Rust GUI 生态、跨语言 native UI 绑定作者，以及需要把
> canvas/GPU 自绘界面暴露给屏幕阅读器和辅助技术的团队。对 open-gpui 最相关的用户不是普通应用开发者，而是框架层、组件 primitive 维护者、测试工具作者和需要把可访问性做成工程门禁的桌面产品团队。

#### `primary_value_proposition`

> 核心价值是把平台可访问性 API 的差异收敛为一棵稳定 ID 驱动的 accessibility tree，并通过完整树初始化加增量 TreeUpdate 的方式让平台 adapter 持有 retained a11y 状态。它与
> open-gpui 的匹配度很高：GPUI 自绘、GPU scene、Element/Entity 和复杂桌面控件都需要一个与渲染树解耦但可稳定追踪的语义树；AccessKit 已经提供
> role、label、value、action、relationship、focus、bounds、text、table/list 等基础语义和跨平台 adapter，可作为 open-gpui a11y contract 的目标后端。

### 分发与生态

#### `distribution_model`

> 分发以 Cargo package dependency 为主，核心 crate 是 `accesskit`，定义
> Node、NodeId、Tree、TreeId、TreeUpdate、Role、Action、ActionRequest、ActionHandler、ActivationHandler 等 schema/API；平台 crate 包括
> `accesskit_windows`、`accesskit_macos`、`accesskit_unix`、`accesskit_android`、`accesskit_ios`，winit 用户可通过 `accesskit_winit`
> 接入；还有 `accesskit_consumer` 用于平台无关树消费和嵌入式辅助技术。项目也维护 C、Python 等语言绑定方向。它不是 copy-to-own、registry、CLI add 或 scaffold
> 模型，而是底层库依赖加平台 adapter 组合。

#### `source_ownership`

> 用户通常不复制 AccessKit 源码，而是依赖上游 crate；toolkit 拥有自己的语义树构建逻辑、稳定 NodeId 策略、focus/action 路由、平台窗口接线和测试夹具。AccessKit 采用宽松开源许可，必要时可以
> fork/patch，但更合理的升级路径是跟随 Cargo SemVer 版本。对 open-gpui 来说，应把 AccessKit 作为 adapter contract，而不是把组件语义直接散落在业务 widget 内；自己的
> `SemanticsSnapshot` 或 a11y layer 应该由 open-gpui 拥有，再可生成 AccessKit TreeUpdate。

#### `rust_distribution_fit`

> 适配度很高。AccessKit 的 crate 拆分符合 Rust 生态：schema 独立、平台 adapter 独立、winit adapter 聚合、consumer crate 复用树逻辑，依赖通过 feature/platform
> 控制，类型由 Rust 强类型表达并可通过 serde、schemars、语言绑定或 JSON Schema 生成跨语言表示。对 open-gpui 的启发是 a11y core 不应依赖窗口和渲染层，平台 adapter 应隔离在后端
> crate，测试应能在无窗口环境构造语义树并验证 TreeUpdate。

### AI 时代设计

#### `machine_readable_contracts`

> AccessKit 的核心就是机器可读契约：Rust 类型定义
> Node、Tree、TreeUpdate、Role、Action、ActionData、TextSelection、bounds、relationship、table/list/text 属性等；serde 可序列化，schemars
> 可生成 JSON Schema，Rustdoc JSON 可供工具读取。它能驱动平台 adapter、consumer、测试查询和跨语言绑定，但不是组件 manifest。open-gpui 应在 AccessKit 之上定义自己的
> component a11y manifest，再把 manifest 编译/测试到 AccessKit tree，避免组件语义只在运行时临时拼装。

#### `copy_modify_verify_loop`

> 典型闭环不是复制 AccessKit 组件源码，而是在 toolkit 中实现语义树生成：为每个可访问元素分配稳定 NodeId，构建 Node
> role/name/value/bounds/children/actions/relationships，初始化完整 Tree，再在 UI 状态变化时推送只包含新增或变化节点的 TreeUpdate，并处理辅助技术发来的
> ActionRequest。验证应包含 `cargo check/test`、无窗口 TreeUpdate 单元测试、平台 adapter 集成测试、screen reader 实测、Accessibility
> Inspector/Orca/KDE inspector 辅助检查，以及 open-gpui 自己的 contract/snapshot/interaction gate。

### API 与组合

#### `api_ergonomics`

> API 是低层基础设施形态，而不是面向应用的声明式组件 API。核心对象是 Node、Tree、TreeUpdate 和 ActionHandler；toolkit 需要显式设置
> role、label、value、bounds、children、focus、actions 和关系属性，并保证更新协议正确。优点是精确、跨平台、可序列化、适合 retained adapter；代价是普通组件作者不应直接手写大量
> AccessKit 节点。open-gpui 应提供更高层的 `Accessible`/`Semantics` builder、组件 part defaults、自动 label 检查和平台诊断，把 AccessKit API
> 封装成框架内部目标格式。

#### `customization_model`

> AccessKit 允许 toolkit 自由决定语义树结构、稳定 ID、哪些视觉元素进入 a11y tree、如何表达 label/description/value/action、如何响应 action
> request、如何映射虚拟化列表和文本选择。它不负责样式、布局算法、组件结构或 interaction policy。open-gpui 的定制边界应是：组件 primitive 声明默认 a11y contract，应用可覆盖
> label/description/action policy，低层 adapter 只消费已验证的语义快照，禁止用户为了视觉调整而破坏必要可访问语义。

#### `component_anatomy_model`

> AccessKit 没有 Root/Trigger/Content/Item/Indicator/Portal 这样的组件 anatomy 模型；它看到的是最终 accessibility
> tree：Window、Button、TextInput、MenuItem、ListBoxOption、Table、TreeItem、Tooltip、Dialog 等 role 和节点关系。对 open-gpui 来说，component
> anatomy 应由组件框架定义，再映射到 AccessKit 节点。例如 Popover 的 Trigger、Content、Arrow、Backdrop、Close 不一定都成为可访问节点，但 Dialog/Menu/Select 的
> active descendant、modal、popup relationship、focus return 和 label 必须有明确语义输出。

#### `state_ownership_model`

> AccessKit adapter 持有当前完整 accessibility tree，toolkit 持有真实 UI 状态和语义快照生成逻辑；辅助技术通过 ActionRequest 请求 focus、click、scroll、set
> value、set text selection、replace selected text、show context menu 等动作，toolkit 再把动作转译为应用状态变化并推送新的 TreeUpdate。TreeUpdate
> 要带最新 focus，更新节点会覆盖旧节点，因此 open-gpui 必须把 a11y 状态视为由应用/组件状态派生的快照，而不是另一个独立真源。

### Headless 与行为

#### `headless_boundary`

> 边界很清楚：AccessKit schema 是 renderer/window neutral 的语义数据层；platform adapters 负责把该语义树暴露给系统 accessibility API；toolkit 负责从自己的
> UI 状态、布局结果、焦点模型和文本模型生成语义树；辅助技术 action 通过 ActionHandler 回到 toolkit。它不处理视觉渲染、theme、layout、hit test 细节、组件状态机或 overlay
> collision。open-gpui 应沿用这个分层，把 a11y tree 作为布局后、渲染前或渲染旁路的确定性产物。

#### `accessibility_model`

> AccessKit 的模型覆盖 role、稳定
> NodeId、children、bounds/transform、focus、label/description/value、disabled/selected/expanded/toggled/checked-like
> state、text selection、scroll offsets、table/list/tree 索引、labelled_by/described_by/controls/owns/active_descendant
> 等关系，以及可由辅助技术请求的 actions。Role 枚举大量借鉴 ARIA/Chromium，当前 schema 很适合作为 open-gpui 的 a11y IR 目标。需要注意官方 README 说明平台 adapter
> 尚未支持所有 UI 元素和所有 schema 属性，已支持非平凡应用和单/多行文本输入，但 rich text 和 hypertext 仍未支持到位。

#### `positioning_and_collision_model`

> AccessKit 不提供 overlay positioning、collision、flip、shift、arrow、安全多边形、dismiss 或 focus return 算法；它只需要最终节点的
> bounds、transform、tree relationship、popup/modal/tooltip/active descendant 等可访问语义。对 open-gpui 的含义是 overlay kernel
> 要独立设计，计算后的浮层几何和焦点关系再输出到 AccessKit。不要把 AccessKit 误当 Floating UI；它能验证浮层是否被辅助技术看到、是否可聚焦、是否有正确关系，但不负责浮层如何定位。

#### `interaction_state_machines`

> AccessKit 不公开 menu/select/dialog/tabs/combobox/table/tree 的 finite state machine；它表达状态机结果，例如
> expanded、selected、active_descendant、text selection、scroll、focus 和支持的 actions，并把辅助技术动作回调给 toolkit。open-gpui 仍需要自己的
> headless state machine 或等价 contract 来定义键盘表、dismiss、typeahead、selection、roving focus、modal focus trap、focus return
> 等行为，然后用 AccessKit tree 作为输出断言之一。

### 渲染与性能

#### `rendering_model`

> AccessKit 没有 DOM/WebView、native retained widget、immediate mode 或 GPU scene 渲染模型。它的运行模型是 retained accessibility
> tree：toolkit 初始推送完整 tree，之后推送增量 TreeUpdate；平台 adapter 保留完整语义树并暴露给系统辅助技术。这个模型反而适合自绘和 immediate-mode GUI，因为视觉层可以每帧重建，但
> a11y 层只要能给出稳定 NodeId 和增量语义更新即可。

#### `performance_model`

> 性能策略是避免平台 adapter 同步拉取 UI 信息，让 toolkit 主动 push 完整初始化和增量更新；只有 adapter 需要保留完整 a11y tree。TreeUpdate
> 文档要求更新只包含新增或变化节点，因为未变化节点重复处理仍有成本；更新必须基于同步的前一状态，错误应用到不匹配的树应立即失败。对 open-gpui 来说，核心性能挑战是稳定
> NodeId、虚拟化列表/表格/树、文本编辑、滚动和焦点变化的增量语义 diff，避免把整个复杂 UI 每帧全量推给 AccessKit。

#### `native_advantage`

> AccessKit 帮 open-gpui 放大 native 优势：无需嵌入浏览器，也能把 GPU 自绘/原生控件暴露给平台辅助技术；可把大文本、表格、树、命令面板、dock、canvas overlay 等高密度桌面 UI
> 映射为平台可访问对象；可直接响应系统级 screen reader action，而不是依赖 DOM/ARIA。open-gpui 若把 AccessKit 作为一等
> contract，可以在性能和可访问性之间建立比很多自绘框架更强的差异化。

#### `web_ecosystem_advantage`

> Web/DOM 的优势仍然很明显：浏览器内建 accessibility tree、ARIA/WAI-ARIA APG 经验、屏幕阅读器兼容矩阵、DevTools accessibility pane、成熟测试工具和大量组件库都比
> native Rust 生态成熟。AccessKit 借鉴 Chromium/ARIA，但不能自动继承浏览器多年兼容性。open-gpui 应复用 Web 术语和行为规范作为设计参考，同时通过 AccessKit
> 输出原生平台语义；对于复杂富文本、HTML 内容、网页嵌入和 webview 场景，应考虑互操作而不是重做整个 Web a11y 生态。

### 主题与设计系统

#### `theme_token_model`

> AccessKit 不提供 theme token、semantic color、spacing、density、motion、state variants 或 runtime theme 解析。它只关心某些可访问相关的视觉/文本属性，例如
> bounds、foreground/background color、font、text decoration、hidden、disabled、focus、selection、scroll 等。open-gpui 的 theme token
> 系统应独立设计，但要把可访问性要求纳入 token gate，例如焦点环、对比度、禁用态可辨识、reduced motion 和高对比模式不能只停留在视觉层。

#### `design_token_pipeline`

> AccessKit 不支持 DTCG、Style Dictionary、Tailwind-like transform 或跨平台 token 输出 pipeline。它可以消费最终语义和部分视觉属性，但不是设计 token 工具。对
> open-gpui 的设计含义是 token pipeline 与 a11y pipeline 要相互校验而非合并：theme artifact 负责颜色/尺寸/状态，AccessKit contract 负责语义树；测试层检查 token
> 选择是否破坏对比度、焦点可见性和屏幕阅读器可理解性。

#### `style_customization_boundary`

> 样式不属于 AccessKit 职责。AccessKit 能承载与辅助技术相关的可见性、文本、颜色、bounds、状态和关系，但不会决定组件视觉结构。open-gpui 应把 style customization 放在 theme
> recipe/component prop/app adapter 层，把 a11y contract 作为不可被样式覆盖破坏的底线。例如用户可以把按钮画成任意样式，但 Button role、label、enabled
> state、default action、focus bounds 和键盘行为不能因为视觉替换而消失。

### 组件表面

#### `component_coverage`

> AccessKit 覆盖的是可访问语义表面而不是可用组件实现。Role 枚举覆盖
> Button、TextInput、CheckBox、RadioButton、Switch、Slider、SpinButton、Menu/MenuItem、ComboBox、ListBox、Table、Tree、TreeGrid、Tab/TabList/TabPanel、Tooltip、Window、Dialog/AlertDialog、ScrollView、Toolbar、Progress、Meter、Terminal
> 等大量角色；Node 属性覆盖文本、表格、滚动、关系和状态。它不提供这些组件的视觉、布局、状态机或交互实现。

#### `must_have_for_open_gpui`

> 对 open-gpui 来说必须补齐，并且应该尽早进入架构核心。最低要求是：定义 renderer-neutral semantics tree；所有基础组件必须声明 AccessKit mapping；布局后产生
> bounds/focus/relationship；ActionRequest 必须能路由回组件/应用状态；测试可以在无窗口环境断言 role/name/value/action/focus；平台后端通过 AccessKit adapter
> 暴露给 OS。没有这个基础，open-gpui 的通用 UI 框架会在 screen reader、键盘导航、AI 操作和长期可维护性上留下结构性缺口。

#### `do_not_chase`

> 不要把 AccessKit 当成完整组件框架、布局系统、overlay kernel、主题系统、Web ARIA 兼容层、screen reader 替代品或自动可访问性魔法。当前阶段也不应为了覆盖全部 schema
> 属性而拖慢核心组件交付；应优先做高价值控件的深契约：Button、TextInput、Checkbox、Radio、Switch、Slider、Menu、Popover/Dialog、Tabs、List/ListBox、Table/Tree、ScrollView、Tooltip、Toolbar。rich
> text/hypertext 等 AccessKit 官方也提示尚未完整支持的领域应谨慎推进。

### 文档测试工具

#### `testing_strategy`

> 测试策略应以 AccessKit 为核心门禁之一，但不能只靠 inspector。官方应用开发者提示强调 screen reader 实测最可靠，也说明 macOS/Linux inspector 可能误导，Linux inspector
> 还受 AT-SPI bus 和工具稳定性影响。open-gpui 应建立多层测试：纯单元测试生成 SemanticsSnapshot/TreeUpdate；contract tests 断言
> role/name/value/action/focus/relationship；interaction tests 驱动键盘和 ActionRequest；平台 smoke tests 覆盖 Windows/macOS/Linux；人工
> screen reader dogfood 作为发布前检查；对虚拟化环境的音频和快捷键问题要有单独说明。

### 治理

#### `versioning_and_breakage`

> AccessKit 通过多个 Cargo crate 独立发布；调研时 docs.rs 显示核心 `accesskit` 为 0.24.1，`accesskit_winit` 和 `accesskit_windows` 为
> 0.33.1，`accesskit_macos` 为 0.26.2，`accesskit_unix` 为 0.22.0，`accesskit_android` 为 0.7.4，`accesskit_ios` 为 0.1.1。多 crate
> 版本不同步意味着 open-gpui 需要锁定兼容矩阵、后端 feature、平台最低版本和 schema 版本，并为 AccessKit breaking change 准备 adapter 层迁移，而不是让业务组件直接依赖具体
> AccessKit 细节。

#### `maintenance_cost`

> 实现和维护成本中高，但这是 native UI 框架不可绕开的成本。open-gpui 需要维护 semantics layer、稳定 ID 策略、layout 到 bounds 映射、文本模型映射、虚拟化策略、ActionRequest
> 路由、平台 adapter glue、无窗口测试、平台 smoke test 和文档。好处是把复杂度集中在框架层后，组件和应用可以获得一致 a11y 基线；坏处是如果 contract 设计过薄，后期补救会比一开始建模更贵。

#### `risks`

> 主要风险包括：把 AccessKit 输出当作事后附加物，导致组件结构和焦点模型无法稳定映射；NodeId 不稳定导致增量更新和 screen reader 体验抖动；虚拟化列表/表格/树只渲染可见项却没有正确表达总数、位置和 active
> descendant；平台 adapter rough feature parity 不等于所有 schema 属性都可用；rich text/hypertext 支持不足会影响编辑器类场景；只依赖 inspector 而不做 screen
> reader 实测会产生假阳性；AI 生成 UI 若没有 a11y contract gate，容易漏 label/action/focus。

#### `open_gpui_relevance`

> 最终建议：adopt。AccessKit 应成为 open-gpui native accessibility contract 的主要目标后端和测试基准，但不要让组件直接裸写 AccessKit 节点。直接设计含义是：新增或明确
> open-gpui 自己的 Semantics/A11y IR；为每个 primitive 定义 AccessKit mapping；把 focus、keyboard、ActionRequest、layout bounds 和 tree
> diff 纳入框架层；平台后端通过 AccessKit adapter 输出；docs/gallery/tests 从同一 contract 派生；对未成熟领域如 rich text/hypertext 标注限制并逐步试点。

### 不确定字段（已跳过）

- `ai_friendliness`
- `diagnostics_and_failure_quality`
- `docs_gallery_model`
- `registry_viability`
- `third_party_ecosystem_path`

## <a id="cargo-crates-io-cargo-generate-xtask-scaffold"></a>26. Cargo / crates.io / cargo-generate / xtask scaffold

- 结果文件：`Cargo_crates_io_cargo_generate_xtask_scaffold.json`
- 调研类别：`rust_distribution_tooling`
- 纳入原因：
  Rust 生态分发与代码生成更自然依赖 Cargo；需要比较它是否比 shadcn registry 更适合 open-gpui。
- 参考来源：
  - https://crates.io/
  - https://doc.rust-lang.org/cargo/
  - https://github.com/cargo-generate/cargo-generate

### 定位

#### `positioning`

> Cargo、crates.io、cargo-generate 与 xtask scaffold 的组合定位不是 UI 组件库或 rendering framework，而是 Rust 生态原生的分发、项目生成、工作区治理和验证自动化层。对
> open-gpui 来说，它更像 scaffold/registry/tooling substrate：Cargo 承载稳定 crate，crates.io 承载公开包分发，cargo-generate 承载新项目或模板生成，xtask
> 承载仓库内契约扫描、gallery 生成和发布前门禁。

#### `target_users`

> 主要服务 Rust 应用开发者、库维护者、框架作者、桌面产品团队、第三方组件作者和 AI agent。对 open-gpui 的关键用户是需要可靠安装 open-gpui crate、生成最小可运行桌面应用、添加官方组件
> recipe、运行本地验证门禁的 Rust 团队。

#### `primary_value_proposition`

> 核心价值是顺着 Rust 生态默认路径做分发与自动化：依赖解析、版本约束、feature flags、workspace、SemVer、cargo add、cargo publish、cargo generate 和 cargo run
> -p xtask 都是 Rust 用户熟悉的入口。它比直接复制 shadcn registry 更适合 open-gpui 的底层分发；但它不能单独解决组件可发现性、源码 recipe、docs/gallery 和 AI
> 可读契约，仍需要一个轻量 metadata registry 配合。

### 分发与生态

#### `distribution_model`

> 推荐采用分层分发模型：稳定运行时能力以 Cargo package dependency 分发，例如 open-gpui、open-gpui-platform、open-gpui-ui-core、open-gpui-ui-
> components、open-gpui-wgpu 等 crate；公开版本发布到 crates.io，私有或实验阶段可用 git/path dependency 或 alternate registry；应用骨架、官方示例和组件起步模板用
> cargo-generate 生成；仓库内增量生成、契约扫描、主题 schema、gallery conformance、发布检查和导入边界检查由 xtask 执行；组件、主题、示例和 AI recipes 的发现层则用 open-gpui
> 自有 JSON/YAML/TOML metadata registry，而不是把所有源码当成 npm 式包分发。

#### `source_ownership`

> Cargo 依赖模式下，用户通常不拥有组件源码，只拥有版本约束和本地应用代码，升级路径清晰，patch/fork 可通过 [patch]、git dependency、path dependency 或 fork crate
> 完成；cargo-generate 生成的项目骨架和示例源码由用户拥有，后续升级类似模板漂移合并；xtask 生成的文件应区分 checked-in artifact 与临时 artifact。与 shadcn copy-to-own
> 相比，Cargo 模式降低日常升级成本，但牺牲深度结构定制；open-gpui 应只对 recipes、示例、主题和应用骨架使用 copy/generate，对核心 primitive 保持 crate API 分发。

#### `registry_viability`

> 对 native Rust UI 来说，不需要复刻 shadcn 的完整源码 registry 作为主分发渠道。Cargo/crates.io 已经是 Rust 的 package registry，适合承载可编译、可版本化、可审计的
> crate。open-gpui 仍然需要 registry，但形态应是 metadata/recipe registry：记录组件名、crate 版本、feature、模块路径、公开 API、resolved-state 契约、a11y
> claim、theme token、gallery story、截图基线、示例源码位置、生成命令和验证命令。也就是说，Cargo 是包 registry，open-gpui registry 是组件事实源和 AI/文档/脚手架索引。

#### `rust_distribution_fit`

> 适配度很高。Cargo workspace 能统一成员、Cargo.lock、target 目录、workspace.package、workspace.dependencies 和根级 patch/profile；features
> 能表达可选平台、renderer、组件族和实验能力；crates.io 发布永久版本，配合 SemVer 管理破坏性变更；cargo add 能添加 registry、path 或 git 依赖；cargo-generate 能从 git
> 模板生成新项目；xtask 能把 cargo fmt、cargo check、cargo nextest、schema scan、import boundary scan 和 UI contract scan 收敛成稳定命令。open-
> gpui 当前仓库已经有 workspace、open-gpui-* crate、xtask verify、scan-theme-drift、scan-import-boundary、scan-ui-contract 和 theme
> schema scan，说明这条路径可直接落地。

### AI 时代设计

#### `ai_friendliness`

> 整体 AI 友好度高于纯文档分发，但低于 shadcn 式源码 registry 的即时可改性。Cargo 元数据、Cargo.toml、workspace 结构、features、crate
> docs、docs.rs、examples、nextest 命令和 xtask 门禁都容易被 AI 检索和验证；cargo-generate 模板能把应用骨架标准化；metadata registry 能告诉 AI 应该添加哪个
> crate、启用哪些 features、复制哪些 recipe、运行哪些验证。短板是 Cargo 本身只描述包依赖，不描述组件 anatomy、状态机、a11y 和 gallery 语义，所以 open-gpui 必须补组件级机器可读
> contract。

#### `machine_readable_contracts`

> Cargo 提供强机器可读基础：Cargo.toml、Cargo.lock、workspace manifest、features、package metadata、dependency graph、SemVer 版本和 crates.io
> API。cargo-generate 模板可通过 cargo-generate.toml、占位符、条件、include/exclude、require version 和脚本钩子表达生成契约。xtask 可以读取源码、schema 和
> manifest 执行项目特定扫描。对 open-gpui 来说，缺的不是包级 schema，而是组件级 typed registry：组件状态、公开 API、source owner、a11y claim、theme
> token、gallery sample、selector、性能预算和验证命令应由同一事实源派生。

### API 与组合

#### `api_ergonomics`

> 作为分发工具链，它不直接规定 UI API；它提供的是 Rust 开发体验上的人体工学：`cargo add open-gpui`、启用 features、`cargo generate` 创建项目、`cargo run -p xtask
> -- verify` 验证、`cargo nextest run -p open-gpui-ui-components` 聚焦测试。映射到 open-gpui 组件 API 时，推荐继续采用 Rust builder pattern、显式
> enum、typed props、resolved-state structs、controlled/default/on_change 语义和 crate-root/prelude exports，而不是为了 registry
> 迁就字符串化组件协议。

#### `customization_model`

> 定制应分层：Cargo features 控制平台、renderer、可选组件族和实验能力；Cargo dependency 版本控制稳定行为；theme token/schema 控制视觉系统；metadata registry 和
> cargo-generate 模板控制项目骨架与默认 recipe；用户源码控制应用结构和业务状态；fork/path dependency 作为深度修改 escape hatch。不要把所有定制都放进 feature
> flags，也不要让模板复制承担核心组件 API 的演进职责。

#### `component_anatomy_model`

> Cargo/cargo-generate/xtask 本身不提供 root/trigger/content/item/indicator/portal 等组件 anatomy，但可以分发和验证这类 anatomy。open-gpui 应把
> anatomy 写入组件 contract registry，例如 Popover 的 trigger/content、Menu 的 item/submenu/separator、Table 的
> header/body/row/cell、Tree 的 item/toggle；Cargo 负责把实现 crate 带进项目，xtask 负责检查 registry、文档、gallery 和 public API 是否对齐。

#### `state_ownership_model`

> 该工具链不定义 controlled/uncontrolled 或 state hoisting，但很适合强化 open-gpui 已有的状态所有权规范。稳定状态模型应留在 crate API 和 renderer-neutral
> resolved state 中；模板只生成初始组合代码；xtask 检查 public API inventory、default seed、controlled runtime input、policy hint、adapter-
> only surface 和 source-owner drift。这样状态所有权不会散落在复制后的组件文件中。

### Headless 与行为

#### `headless_boundary`

> Cargo 分发天然鼓励边界清晰的 crate 拆分：ui-core 承载 renderer-neutral contract，ui-components 承载 GPUI adapter 和官方组件，platform/renderer
> crate 承载系统差异，examples/gallery 承载 dogfood。open-gpui 可以用 workspace 和 import-boundary xtask gate 强制
> headless/adapter/theme/docs 边界，而不是依赖 registry 约定。这个边界比 shadcn 的源码复制模式更容易长期维护。

#### `accessibility_model`

> Cargo 工具链本身不提供 role、focus、keyboard 或 screen reader 模型；它能分发 accesskit 依赖、GPUI adapter crate、a11y contract crate
> 和测试工具。open-gpui 应把 accessibility 作为组件 registry 和 xtask gate 的一等字段：resolved state 暴露 role、label
> source、value、orientation、actions 和 relationship，GPUI adapter 映射到 AccessKit，xtask scan-ui-contract 检查 a11y claims 与
> gallery evidence 对齐。

#### `positioning_and_collision_model`

> 该组合不解决 overlay positioning 或 collision；它只决定如何分发和验证相关实现。open-gpui 的 overlay geometry contract 应在 ui-core/ui-components
> 中实现，Cargo features 或 crate 分层控制可选能力，registry 条目记录支持的 placement、anchor、safe bounds、focus restore 和 dismiss
> policy，xtask/golden tests 验证 flip、shift、size、arrow、submenu corridor 等行为。不要期待 cargo-generate 模板替代核心定位算法。

#### `interaction_state_machines`

> Cargo/cargo-generate/xtask 不提供有限状态机，但能把状态机作为可测试 contract 固化。open-gpui 应把 menu、select、combobox、dialog、tabs、tree、table
> 等交互状态写成 Rust 类型和测试数据，再由 xtask 检查 contract、docs、gallery sample、runtime smoke 和 public API 是否一致。对 AI 来说，状态机不应藏在模板里，而应成为
> crate 内可编译、可测试的事实源。

### 渲染与性能

#### `rendering_model`

> 该对象没有渲染模型；它是 Rust 构建、分发、模板和自动化层。open-gpui 的渲染仍由 native retained/entity、WGPU/GPU scene、平台 backend、deferred overlay 和 GPUI
> adapter 决定。Cargo 只负责把这些 crate 正确组合到应用里。

#### `performance_model`

> 性能价值主要在工程层而不是运行时层：Cargo workspace 统一构建图，feature flags 减少不必要依赖，nextest 提升测试执行体验，xtask 把性能或 gallery smoke
> 纳入门禁，crates.io/docs.rs 降低安装和文档发现成本。它不会直接优化大列表、大表格、大树、文本、canvas 或 overlay 布局；这些仍要由 open-gpui 的
> virtualizer、table/tree/text/canvas/overlay primitive 承担。

#### `native_advantage`

> 这种 Rust-native 分发路径能放大 GPUI 的原生优势：用户安装的是真正的 native crate，不需要 WebView/npm bundler；平台 backend、WGPU
> renderer、AccessKit、文本和虚拟化可以按 feature/crate 分层；桌面应用可以通过 Cargo workspace 与自身业务 crate
> 深度集成。它特别适合高密度桌面应用、开发工具、金融终端、canvas、table/tree 和长期运行进程。

#### `web_ecosystem_advantage`

> Web/Tauri/Electron 生态在组件市场、copy-to-own 模板、可视化 block、CSS/token 工具链、Storybook、浏览器可访问性和 AI 训练语料上更成熟。Cargo 生态在 UI 组件可发现性、截图
> gallery、交互 story 和设计系统工具链上弱很多。open-gpui 不应拒绝 Web 生态经验，而应吸收 shadcn/Storybook 的 metadata、gallery 和 AI 入口，同时让运行时和分发基线保持
> Cargo-native。

### 主题与设计系统

#### `theme_token_model`

> Cargo 不定义 theme token，但能分发主题 crate、主题 JSON、schema 生成器和验证工具。open-gpui 当前已具备 theme schema scan 和 theme drift scan 的雏形，应把
> theme token 设计成版本化 schema：颜色、语义 token、状态 token、密度、圆角、字体、阴影、fallback mode、light/dark/high-contrast 等由 crate API 和 JSON
> schema 共同约束；registry 条目记录组件消费哪些 token，xtask 检查 drift。

#### `style_customization_boundary`

> 样式责任应明确切分：core crate 不绑定视觉样式；ui-components 提供默认 theme recipe 和 typed token intent；Cargo features 不承担视觉变体爆炸；cargo-
> generate 只生成初始主题文件或示例；用户应用拥有最终 theme definition 和局部组合代码；xtask 检查 schema、token vocabulary、component contract 和 gallery
> evidence。这样可避免 shadcn/Tailwind 式 class 定制模型被误搬到 native GPUI。

### 组件表面

#### `component_coverage`

> 该对象不覆盖具体 UI 组件。它覆盖的是组件生态增长所需的基础设施：crate 发布、依赖添加、workspace 治理、模板生成、代码生成、验证门禁、schema 导出、契约扫描、示例构建和发布流程。

#### `must_have_for_open_gpui`

> 对 open-gpui 来说是必须补齐的基础能力：稳定 crate 分层和发布顺序、Cargo features 策略、cargo add 友好的 README/docs、cargo-generate 官方应用模板、xtask
> verify/scan-ui-contract/scan-theme-schema/scan-theme-drift 的持续强化、组件 metadata registry、gallery/story 与 contract 同源、以及 AI
> 可执行的 add/verify 指令。没有这些，组件库即使实现很多 widget，也难形成可维护生态。

#### `do_not_chase`

> 不要追逐 npm 式组件包市场、把每个按钮/菜单拆成独立 crate、复杂私有 registry、模板内嵌大量业务逻辑、feature flags 爆炸、宏生成不可读组件源码、过早构建 shadcn 完整 diff/add 云服务、或把
> xtask 变成无边界的大杂烩。当前阶段应优先让少量官方组件、主题和 gallery 契约在 Cargo-native 流程中跑通。

### 文档测试工具

#### `docs_gallery_model`

> 推荐 docs、gallery、examples、AI examples 和 scaffold 从同一组组件 contract/registry 派生。Cargo docs 和 docs.rs 负责 API 文档；examples/ui-
> foundation-gallery 负责真实运行样例；metadata registry 记录组件状态、selectors、a11y claims、story contracts 和验证命令；xtask 负责检查 docs
> token、gallery conformance、theme schema 和 public API drift；cargo-generate 负责把最小应用和官方样例落到新项目。这个模型比单纯 Markdown 或单纯
> crates.io 更适合 AI 和长期治理。

#### `testing_strategy`

> 测试策略应以 Cargo/nextest/xtask 为骨架：unit tests 验证 ui-core 纯逻辑；component contract tests 验证 resolved state、API inventory 和
> source ownership；runtime tests 验证 GPUI adapter、focus、overlay、keyboard 和 scroll；gallery smoke 验证真实样例；a11y tests 验证
> role/label/value/action；schema drift tests 验证 theme 和 registry；import boundary gate 防止层级回退；发布前运行 cargo fmt、cargo check
> --workspace、cargo nextest 和 xtask verify。open-gpui 当前已有多项相符基础。

#### `diagnostics_and_failure_quality`

> Cargo/rustc 的类型错误、feature resolution 错误、SemVer 约束和 test failure 已经有较强诊断；xtask 可以进一步把失败提升到组件语义层，例如指出组件名、registry
> row、source owner、文档 token、gallery selector、a11y claim、theme token、schema artifact 和建议命令。对 AI 自动修复来说，关键不是更多模板，而是让 xtask
> failure 以稳定、结构化、可定位的方式输出。

### 治理

#### `versioning_and_breakage`

> Cargo/crates.io 的版本治理是最大优势：发布版本永久不可覆盖，SemVer 是社区默认语言，Cargo.lock 固定应用依赖，workspace.dependencies 统一内部版本，feature flags
> 表达兼容能力，cargo publish/check 可形成发布顺序。open-gpui 应把核心 crate 作为 SemVer 主体，把组件 registry schema、theme schema、gallery contract 和
> template version 作为独立版本面治理；copy-to-own recipe 必须明确漂移和迁移策略，不能假装能用普通 crate upgrade 解决。

#### `maintenance_cost`

> 维护成本中等但可控。Cargo/crates.io 本身由生态承担，open-gpui 主要维护 crate 边界、features、发布顺序、模板、xtask 命令、registry schema、docs/gallery
> 派生和迁移说明。相比自建 shadcn 式完整 registry/CLI/diff 生态，短期成本更低、Rust 用户学习成本更低；相比只发布 crate，成本更高，因为要维护 metadata、gallery、AI 和验证闭环。

#### `risks`

> 主要风险是把 Cargo 当成万能 registry，导致组件可发现性和 AI 可读性不足；feature flags 过多造成解析复杂和组合爆炸；每个组件独立 crate 造成版本碎片化；cargo-generate
> 模板漂移后难升级；xtask 逻辑增长失控；crates.io 发布永久不可覆盖，错误版本治理成本高；第三方组件如果只发 crate 而没有 contract/gallery/a11y metadata，会比 shadcn copy-to-
> own 更难理解和修复。

#### `open_gpui_relevance`

> 最终建议：adopt。open-gpui 应把 Cargo/crates.io/workspace/features/SemVer 作为主分发底座，把 cargo-generate 作为新项目和示例 scaffold，把 xtask
> 作为本仓库 contract、theme、gallery 和发布门禁，把轻量 open-gpui metadata registry 作为组件事实源。直接设计含义是：不要照搬 shadcn 的源码 registry 作为核心分发模型；应采用
> Rust-native mixed model，即稳定能力发布为 crate，recipes/templates 可生成或复制，组件发现和 AI contract 由机器可读 registry 驱动。

### 不确定字段（已跳过）

- `copy_modify_verify_loop`
- `design_token_pipeline`
- `third_party_ecosystem_path`

## <a id="ai-era-component-distribution"></a>27. AI-era component distribution

- 结果文件：`AI_era_component_distribution.json`
- 调研类别：`ai_native_ecosystem_design`
- 纳入原因：
  研究组件库如何提供 machine-readable metadata、recipes、tests、examples、constraints，让 AI 可靠生成和修改 native UI。
- 参考来源：
  - https://ui.shadcn.com/docs
  - https://storybook.js.org/docs

### 定位

#### `positioning`

> AI-era component distribution 的定位不是单个组件库，而是面向 AI agent、应用开发者和设计系统团队的组件分发与验证协议。它结合 shadcn/ui 的 open code、copy-to-
> own、CLI、registry schema、MCP 与 skills，以及 Storybook 的 stories、args、autodocs、manifests、interaction tests、a11y tests、visual
> tests 和 MCP，把组件源码、用法、约束、示例和测试变成可机器读取、可本地修改、可自动验证的生态层。

#### `target_users`

> 主要服务原生 UI 框架作者、Rust 桌面应用团队、设计系统维护者、第三方组件作者、文档/测试工具维护者和 AI 编程 agent。对 open-gpui 来说，最直接的用户是需要快速生成、改造和验证 GPUI
> 原生组件的框架维护者与产品工程师。

#### `primary_value_proposition`

> 核心价值是把组件从“人读文档后手工集成”升级为“AI 可检索、可组合、可落地、可验证”的分发单元。它与 open-gpui 高度匹配：open-gpui 如果只提供 Rust API 和 examples，很难形成 AI 可靠生成
> native UI 的闭环；如果提供 registry manifest、源码 recipe、story manifest、contract tests 和 verify 命令，就能把原生性能优势转化为可增长生态。

### 分发与生态

#### `source_ownership`

> 组件源码应尽量 copy-to-own：应用团队拿到可读、可改、可审查的 Rust 组件文件和 recipe，而不是只能依赖黑盒 widget API。升级成本会从普通 crate 升级变成 diff/merge/verify 成本，因此
> registry 必须记录生成来源、版本、文件目标、依赖 crate、feature、局部修改标记和迁移说明。框架核心 primitive 仍由 crate 维护，避免每个应用复制状态机、可访问性和底层性能逻辑。

### AI 时代设计

#### `ai_friendliness`

> AI 友好度应成为一等目标。shadcn 的 open code、consistent API、components.json、registry schema、llms.txt、skills 和 MCP 让 AI
> 能找到、安装、理解和修改组件；Storybook 的 stories、args、parameters、JSDoc、autodocs、manifests、MCP 与 run-story-tests 让 AI 能验证生成结果。open-gpui
> 应把每个组件发布为源码 + typed metadata + usage examples + constraints + verification plan，而不是只给一段 prose 文档。

#### `copy_modify_verify_loop`

> 推荐闭环是：gpui add 从 registry 拉取 recipe 和 metadata，写入本地源码、Cargo 依赖、theme token、stories 和 tests；开发者或 AI 修改组件源码；gpui diff
> 显示本地与上游差异；gpui verify 运行 cargo fmt、cargo nextest、story render smoke、交互脚本、AccessKit 树断言、截图对比、性能 smoke 和 schema drift
> gate；失败报告返回组件、story、文件、token、节点、步骤和建议修复层。这个闭环比单纯 copy/paste 更适合 AI 自动修复。

### API 与组合

#### `customization_model`

> 定制应分层：core primitive 提供行为和语义，theme token 提供颜色/间距/字体/圆角/状态/密度，recipe 提供默认视觉和布局，组件 props 提供有限变体，本地源码提供最终 escape hatch，app
> adapter 处理平台差异。AI 修改时应优先改 recipe、token 或局部源码，并通过 manifest 知道哪些字段是稳定 API、哪些是内部实现。

#### `component_anatomy_model`

> 复杂组件必须显式 anatomy 化。menu、select、dialog、popover、tabs、combobox、table、tree 等不应只是一个黑盒 widget，而应声明
> root、trigger、content、item、group、separator、indicator、viewport、portal、overlay、arrow、label、description 等 parts。这个模型利于 AI
> 组合和局部修改，也利于 gallery 生成每个 part 的文档、截图和可访问性断言。

### Headless 与行为

#### `headless_boundary`

> AI-era 分发要求 headless 边界更硬：行为逻辑、状态机、a11y metadata、layout/positioning、render adapter、style/theme 和 docs/tests
> 应能独立演进。shadcn 的优势是源码可改和组合一致，弱点是行为、DOM、Tailwind 和 React adapter 绑定较深；Storybook 的优势是独立记录状态和验证，但不定义行为本身。open-gpui 应把
> behavior primitive 与 visual recipe 分开，再让 registry 把二者组合成可安装组件。

### 渲染与性能

#### `rendering_model`

> 参考对象本身偏 Web：shadcn 输出 React/DOM/Tailwind 源码，Storybook 主要在浏览器预览、索引、截图和测试。open-gpui 的目标渲染模型应是原生 retained/GPUI Element +
> GPU scene + 增量渲染 + 原生窗口/输入/可访问性。AI-era 分发层不应决定渲染内核，只负责把可安装组件、story 和验证映射到该 native renderer。

#### `native_advantage`

> native GPUI 应在低延迟输入、大文本编辑、大列表/树/表格、复杂 docking、多窗口、多显示器、高 DPI、原生菜单快捷键、GPU 绘制、长期运行内存占用和 AccessKit 集成上形成优势。AI-era
> 分发的价值是把这些优势包装成可复用 primitive 和可验证 recipes，而不是用 Web 风格组件数量竞争。

#### `web_ecosystem_advantage`

> Web 生态在组件数量、表单、图表、营销 blocks、Storybook/Chromatic 云端评审、浏览器调试、CSS token、第三方库和设计工具联动上天然更强。open-gpui 应参考 shadcn/Storybook
> 的分发、metadata 和测试机制，但短期避开完整 Web 组件宇宙、MDX 文档平台、Chromatic SaaS 复刻和 Tailwind class 生态；必要时输出 Web 可读报告或静态 gallery 互操作。

### 主题与设计系统

#### `style_customization_boundary`

> 样式责任边界应明确：framework primitive 不绑定具体视觉；official theme recipe 给出默认视觉；registry item 可以携带 token 需求、默认 recipe 和局部样式；组件 prop
> 只暴露稳定变体；用户源码可最终覆盖；gallery/story 只组合和验证，不成为运行时样式依赖。这样 AI 才能判断应该改 token、recipe、prop 还是源码。

### 组件表面

#### `component_coverage`

> AI-era 分发关注的不是组件数量本身，而是组件是否有源码、metadata、recipes、examples、stories、tests 和
> constraints。覆盖面应从基础控件、form、overlay、navigation、data display、feedback、application shell 扩展到
> table/tree/text/docking/command palette 等原生桌面高价值组件；每个组件还应覆盖默认、禁用、错误、加载、空状态、长文本、键盘焦点、主题、尺寸和平台差异。

#### `must_have_for_open_gpui`

> 必须补齐。open-gpui 通用 UI 框架至少需要：registry schema、components manifest、gpui add/diff/verify、源码 recipe、typed props/anatomy
> metadata、theme token schema、story/gallery runner、AI 可读 manifests、MCP 或等价查询接口、interaction scripts、AccessKit
> snapshots、visual golden、performance smoke 和结构化 diagnostics。没有这些，AI 生成 native UI 会停留在“能编译但不可置信”的水平。

#### `do_not_chase`

> 当前阶段不应追逐完整 shadcn Web 组件目录、Tailwind 语法体系、Next.js/RSC 安装矩阵、营销 blocks、完整 Storybook addon 市场、MDX 文档平台、Chromatic
> 云协作产品、浏览器矩阵和设计工具全链路。open-gpui 应先实现最小但强约束的 native 分发闭环，再逐步开放第三方 directory 和更丰富的 gallery。

### 文档测试工具

#### `docs_gallery_model`

> docs、gallery、examples、manual dogfood、AI examples 和 tests 应尽量从同一事实源派生。shadcn 的 registry schema 和 Storybook 的
> story/autodocs/manifest/MCP 共同说明：人看的文档、AI 查的 manifest、CLI 安装的信息、gallery 展示的状态、测试运行的 fixture 不应分裂。open-gpui 可以以 registry
> item + story manifest 为中心生成文档页、示例代码、组件目录、MCP 查询结果、截图矩阵和验证计划。

#### `diagnostics_and_failure_quality`

> 失败诊断必须面向 AI 自动修复设计。报告应包含 registry item、组件名、story id、args/fixture、theme、platform、viewport/DPI、文件路径、Cargo feature、失败命令、截图
> diff 区域、AccessKit 节点路径、交互步骤、token 名称、期望/实际值和建议修复层级。Storybook/Chromatic 的 story 级定位和 visual diff 很有启发，但 open-gpui 还需要
> native 语义树、性能和编译诊断。

### 治理

#### `maintenance_cost`

> 维护成本高，但这是建立可靠 AI-native UI 生态的必要基础设施。核心团队需要维护 schema、CLI、registry、gallery
> runner、MCP、docs、默认主题、核心组件、测试基线、跨平台截图稳定性和迁移说明；社区需要维护第三方 items、stories 和兼容矩阵。收益是文档、示例、分发、测试和 AI 上下文合一，长期降低组件库演进和大规模重构风险。

#### `risks`

> 主要风险包括：过度复刻 Web/Tailwind/Storybook 形态导致 native 架构错位；schema 过宽变成不可维护平台，过窄又无法表达状态和可访问性；copy-to-own 带来升级漂移和组件碎片化；第三方
> registry 质量参差；AI 生成代码绕过性能和 a11y contract；截图在跨平台字体、DPI、GPU 和动画上噪声过高；早期投入过大拖慢 GPUI primitive 建设。

#### `open_gpui_relevance`

> 建议 adopt 核心思想、trial 最小闭环。直接设计含义是：open-gpui 应把 AI-era component distribution 作为生态基础设施，而不是后期文档补丁。第一阶段可实现 8 到 12 个核心组件的
> registry item、theme token、story manifest、gpui add/diff/verify、gallery 截图和 AccessKit 断言；第二阶段接 MCP、第三方 directory 和更完整的
> performance/visual gate；暂缓云端协作和大型市场。

### 不确定字段（已跳过）

- `accessibility_model`
- `api_ergonomics`
- `design_token_pipeline`
- `distribution_model`
- `interaction_state_machines`
- `machine_readable_contracts`
- `performance_model`
- `positioning_and_collision_model`
- `registry_viability`
- `rust_distribution_fit`
- `state_ownership_model`
- `testing_strategy`
- `theme_token_model`
- `third_party_ecosystem_path`
- `versioning_and_breakage`

## <a id="hybrid-registry-model"></a>28. Hybrid registry model

- 结果文件：`Hybrid_registry_model.json`
- 调研类别：`open_gpui_candidate_architecture`
- 纳入原因：
  > 重点假设：open-gpui 可能不需要 shadcn 式源码 registry，而需要 crate + metadata registry + scaffold recipes + gallery/contract gate。deep
  > 阶段要验证。

### 定位

#### `positioning`

> open-gpui 的候选架构模式，不是外部框架：以 Rust crate 作为代码分发主干，以 typed component_contract、theme schema、gallery metadata、xtask scanner
> 作为机器可读事实源，再用 scaffold recipe 补足 AI/模板分发能力。

#### `target_users`

> 主要服务 open-gpui 应用开发者、组件库维护者、设计系统作者和 AI coding agent；第三方作者可以通过 crate、recipe 或 metadata 进入生态，而不是直接复制一组 Web 风格组件文件。

#### `primary_value_proposition`

> 在保留 Cargo/SemVer/native 性能优势的同时，提供 shadcn 式可发现性和 AI 可组合性；核心价值是让组件契约、文档、gallery、theme schema、a11y claim 和验证门禁共享同一组事实源。

### 分发与生态

#### `distribution_model`

> 建议采用混合分发：稳定组件走 crates.io/Cargo feature；官方示例和应用集成走 examples/gallery；可选扩展走 scaffold recipe；组件元数据走仓库内 JSON/YAML 或 Rust
> typed registry；主题走 schema artifact。源码 copy-to-own 只适合 demo 或 app-local recipe，不应成为主分发模型。

#### `source_ownership`

> 核心组件源码由 crate 维护，用户依赖公开 API；用户可通过 recipe 复制应用层组合代码，但不应复制核心组件实现。这样升级路径清楚，安全/a11y/performance 修复可通过 crate 发布，而不是散落到用户项目。

#### `registry_viability`

> registry 有必要，但不应是 shadcn 式源码 registry 的简单移植。更合适的是 metadata
> registry：记录组件名称、owner、family、gallery_status、docs_token、source_home、default_export、a11y claim、theme evidence、scaffold
> recipe 和验证命令。源码仍由 Cargo 管理。

#### `rust_distribution_fit`

> 与 Rust 生态高度匹配：现有 component_contract 已是 typed registry，xtask 已可验证 registry/export/docs/gallery/theme 对齐。Cargo
> 负责版本和依赖，xtask 负责项目内 scaffold、schema drift、contract audit；比 npm/copy-to-own 更符合 Rust 维护习惯。

### AI 时代设计

#### `ai_friendliness`

> 强。AI 可以读取 registry/schema/contract/test/galleries 来理解组件边界；比纯 Rust API 文档更适合生成。关键是把每个组件的 usage、required imports、theme
> tokens、a11y claims、state ownership、example selectors 和 verification command 都结构化。

#### `machine_readable_contracts`

> 当前已有基础：ComponentContractEntry、SurfaceGalleryStatus、SurfaceDocsStatus、PublicSurfaceOwnerClass、theme JSON
> schema、COMPONENT_A11Y_CLAIMS、COMPONENT_CONFORMANCE_GATES、scan-ui-contract。下一步应导出稳定 JSON manifest，供 docs/scaffold/AI 读取。

#### `copy_modify_verify_loop`

> 推荐流程是 copy recipe, not copy primitive。AI 或开发者复制应用层组合代码后，通过 cargo check、cargo test、scan-ui-contract、scan-theme-
> schema、gallery smoke 验证。核心组件修改仍在 crate 内完成并由完整 verify gate 保护。

### API 与组合

#### `api_ergonomics`

> 应保持 Rust builder/typed state API，同时借鉴前端组件库的命名和 anatomy 心智。用户体验应像 Button/Dialog/Select/Table/Tooltip 等通用组件，而内部通过 typed
> state、callbacks、policy_hints 和 gpui_adapter 隐藏 GPUI 细节。

#### `customization_model`

> 推荐三层定制：theme token/recipe 处理视觉一致性；component props 和 policy hints 处理行为差异；scaffold recipe 处理应用级组合。避免让用户 fork
> 核心组件来改样式，除非是实验性组件。

#### `component_anatomy_model`

> 复杂组件应显式声明 anatomy，例如 root/trigger/content/item/indicator/viewport/overlay/portal-like adapter；在 Rust 中可用 typed
> subcomponents、builder slots 或 state readout 表达。registry 应记录这些 parts 和 source_home。

#### `state_ownership_model`

> 应延续当前方向：public resolved-state 保持 renderer-neutral，应用拥有业务状态，component runtime 拥有临时交互状态，GPUI-specific handles 放在
> gpui_adapter。registry 应把 controlled_inputs、default_seeds、callbacks、policy_hints 纳入可查询清单。

### Headless 与行为

#### `headless_boundary`

> 混合模型要求 headless boundary 先以 crate 内 contract 稳定，而不是急着拆新 crate。overlay policy、roving focus、listbox navigation、scroll
> viewport intent、splitter constraints 等可继续作为 renderer-neutral candidates。

#### `accessibility_model`

> 应把 AccessKit/GPUI 映射留在 adapter，component contract 只声明
> Role、Toggled、Orientation、AccessibleAction、label_source、value_kind、relationship。当前 COMPONENT_A11Y_CLAIMS 可作为 manifest 的
> a11y 子表继续扩展。

#### `positioning_and_collision_model`

> overlay/tooltip/popover/menu 应把定位和碰撞抽象成 neutral geometry/policy contract；Floating UI 算法可作为参考或 golden test，但最终应输出 GPUI-
> native placement、safe margin、focus return、dismiss policy，而非 DOM 依赖。

#### `interaction_state_machines`

> 应在 metadata 中记录关键状态机能力：open/close、dismiss reason、focus return、typeahead、roving focus、selection、scroll ownership、resize
> constraints。实现可不使用显式 FSM 库，但测试和 manifest 要像 FSM 一样可枚举。

### 渲染与性能

#### `rendering_model`

> native retained GPUI + Rust typed components。registry 本身不参与渲染，只描述组件契约和生成 docs/scaffold/testing 所需信息；渲染仍由 crate 中的 GPUI
> component 实现。

#### `performance_model`

> 性能门禁应成为 registry/scaffold 的一等字段：哪些组件必须用 virtualization/lazy mount、哪些 sample 需要 nested scroll containment、哪些
> table/tree/text/canvas 场景需要 smoke/perf gate。当前 gallery 已对 Table/VirtualizedList/Tree 有较强基础。

#### `native_advantage`

> 大表格、大树、虚拟列表、复杂 overlay/focus、文本编辑、canvas/docking 是 native GPUI 应重点展示的差异化场景。metadata registry 应服务这些复杂场景，而不只是列出按钮和输入框。

#### `web_ecosystem_advantage`

> Web/Tauri/Electron 在前端生态、CSS/theme、npm 组件、设计师协作、热更新和第三方模板方面天然更强。open-gpui 不应在源码 registry 数量上硬拼，而应通过性能、Rust 类型、native
> integration 和可验证 contract 取胜。

### 主题与设计系统

#### `theme_token_model`

> 应继续使用 schema_version、ThemeMode、TokenKey、ColorState、ThemeRegistry、theme_json_schema 和 committed schema artifact。registry
> 可记录组件使用的 theme recipe/token coverage，并通过 scan-theme-schema/scan-theme-drift 防漂移。

#### `style_customization_boundary`

> 样式边界应是 theme recipe 优先、component prop 其次、app recipe 最后。核心组件不应要求用户 fork 源码改颜色；但 scaffold recipe 可以生成 app-local
> layout/composition。

### 组件表面

#### `component_coverage`

> 混合 registry 不直接决定覆盖度，但可让覆盖规划可查询。当前 open-gpui 已覆盖基础控件、overlay、table/tree/virtualized list、theme/a11y；可继续补
> date/color/pagination/spinner 等，但应先补 contract metadata 和 scaffold pipeline。

#### `must_have_for_open_gpui`

> 必须补的是 machine-readable public manifest、recipe/scaffold schema、registry-to-docs/gallery 派生、third-party compatibility
> rules、Floating UI-like positioning contract、AccessKit-aware a11y manifest。

#### `do_not_chase`

> 不应盲目追 shadcn 的源码复制模型，也不应把 gpui-component 的 title bar/webview/settings/editor/story crate 全量搬过来。应用级 shell、editor、webview
> 应另开产品化判断，不属于组件 registry 第一阶段。

### 文档测试工具

#### `docs_gallery_model`

> docs/gallery 应逐步从同一事实源派生：component_contract 负责 ownership/export/docs/source，gallery manifest 负责 samples/selectors/state
> probes，theme schema 负责 token vocabulary，a11y claims 负责 role/action/value。避免手写多份清单。

#### `testing_strategy`

> 现有 xtask verify 是正确方向：cargo fmt/check/test/nextest、scan-theme-drift、scan-import-boundary、scan-ui-contract。下一步应增加
> registry JSON export 的 golden test、recipe scaffold smoke test、AI sample compile test。

#### `diagnostics_and_failure_quality`

> 必须保持当前 scanner 风格：失败信息包含文件、组件、token、owner、修复方向。AI 修复依赖这种诊断质量；metadata registry 的验证器也应输出同样具体的错误，而不是泛泛 schema validation
> failed。

### 治理

#### `versioning_and_breakage`

> crate API 走 SemVer；metadata schema 需要独立 schema_version；recipe manifest 需要 min_open_gpui_version；copy-to-own recipe
> 需要注明不会自动升级。breaking change 应附 migration recipe 和 verifier。

#### `maintenance_cost`

> 混合模型初期成本高于 crate-only，因为要维护 typed registry、JSON export、recipe schema、docs/gallery 派生和 validators；但长期低于 copy-to-own
> registry，因为核心 bugfix/a11y/perf 修复仍集中在 crate。

#### `risks`

> 主要风险是过度工程化 metadata、registry 与 Rust API 漂移、第三方 recipe 兼容难、AI 生成错误被误认为官方支持、以及为了追前端生态而牺牲 native 简洁性。必须用小切片试点控制风险。

#### `open_gpui_relevance`

> 建议 trial：不要做 shadcn 式源码 registry；先做 hybrid registry MVP。范围为导出当前 component_contract/theme/a11y/gallery 的 machine-readable
> manifest，增加一个 scaffold recipe 示例，并让 xtask 验证 manifest 与源码一致。

### 不确定字段（已跳过）

- `design_token_pipeline`
- `third_party_ecosystem_path`
