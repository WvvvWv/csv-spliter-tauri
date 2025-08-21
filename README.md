# CSV Splitter - Tauri桌面应用

一个基于Tauri框架开发的跨平台CSV文件分割工具，提供简洁直观的图形界面，支持按指定行数将大型CSV文件分割成多个小文件。

## 🎯 功能特性

- 📁 **文件选择**：直观的文件选择器，支持拖拽和点击选择
- 📊 **智能预览**：自动检测并显示CSV文件的前几行数据
- ⚙️ **灵活配置**：
  - 自定义分割行数
  - 选择是否包含标题行
  - 指定输出目录
- 🚀 **高效处理**：基于Rust的高性能文件处理
- 💻 **跨平台**：支持Windows、macOS、Linux
- 🎨 **现代界面**：响应式设计，支持深色/浅色主题

## 🛠️ 技术栈

### 前端
- **框架**：纯HTML5 + CSS3 + JavaScript (ES6+)
- **样式**：现代CSS Grid和Flexbox布局
- **图标**：SVG矢量图标
- **交互**：原生DOM操作，无额外依赖

### 后端
- **运行时**：Tauri (Rust)
- **核心语言**：Rust
- **文件处理**：标准库文件I/O
- **CSV解析**：手动解析确保性能

### 构建工具
- **包管理**：Cargo (Rust) + npm (开发依赖)
- **打包**：Tauri Bundle
- **安装器**：WiX Toolset (Windows)

## 📦 安装

### 预编译版本
1. 前往 [Releases](../../releases) 页面
2. 下载对应平台的安装包
3. 运行安装程序完成安装

### 从源码构建

#### 环境要求
- **Rust**：1.70.0 或更高版本
- **Node.js**：18.0.0 或更高版本
- **系统依赖**：
  - Windows: Microsoft Visual Studio C++ Build Tools
  - macOS: Xcode Command Line Tools
  - Linux: build-essential, libwebkit2gtk-4.0-dev, libssl-dev

#### 构建步骤
```bash
# 克隆仓库
git clone <repository-url>
cd csv-splitter-tauri

# 安装依赖
cargo install tauri-cli

# 开发模式运行
cargo tauri dev

# 构建发布版本
cargo tauri build
```

## 🚀 使用指南

### 基本使用
1. **启动应用**：双击桌面图标或从开始菜单启动
2. **选择文件**：
   - 点击"选择CSV文件"按钮
   - 或拖拽CSV文件到应用窗口
3. **配置选项**：
   - 设置每个文件包含的行数
   - 选择是否保留标题行
   - 选择输出目录（默认为原文件同目录）
4. **开始分割**：点击"开始分割"按钮
5. **查看结果**：分割完成后自动打开输出目录

### 高级功能
- **批量处理**：支持同时选择多个CSV文件
- **自定义命名**：输出文件自动编号，格式为`原文件名_序号.csv`
- **进度显示**：实时显示处理进度和剩余时间
- **错误处理**：详细的错误提示和日志记录

## 📁 项目结构

```
csv-splitter-tauri/
├── src/                    # 前端源码
│   ├── index.html         # 主界面
│   ├── main.js           # 前端逻辑
│   ├── styles.css        # 样式文件
│   └── assets/           # 静态资源
├── src-tauri/             # 后端源码
│   ├── src/
│   │   ├── main.rs       # 主程序入口
│   │   └── lib.rs        # 核心逻辑
│   ├── icons/            # 应用图标
│   ├── tauri.conf.json   # Tauri配置
│   └── Cargo.toml        # Rust依赖配置
├── .vscode/              # VS Code配置
└── README.md             # 项目说明
```

## 🔧 开发指南

### 前端开发
- 使用原生JavaScript，无构建步骤
- 支持热重载：修改文件后自动刷新
- 样式使用现代CSS特性，支持响应式设计

### 后端开发
- Rust代码组织清晰，模块化设计
- 使用标准库进行文件操作，确保跨平台兼容性
- 完善的错误处理和日志记录

### 调试技巧
```bash
# 查看Rust日志
$env:RUST_LOG="debug"
cargo tauri dev

# 前端调试
# 按F12打开开发者工具
# 在Console中查看日志输出
```

## 🐛 常见问题

### Q: 应用无法启动？
A: 检查系统是否安装必要的运行时依赖：
- Windows: Visual C++ Redistributable
- macOS: 允许来自未知开发者的应用
- Linux: 安装webkit2gtk相关包

### Q: 大文件处理很慢？
A: 尝试：
1. 减少单个文件行数
2. 关闭实时预览功能
3. 确保有足够的磁盘空间

### Q: 输出文件编码问题？
A: 应用默认使用UTF-8编码，如需其他编码请在设置中调整

## 🤝 贡献指南

欢迎提交Issue和Pull Request！

### 开发流程
1. Fork 项目
2. 创建功能分支 (`git checkout -b feature/AmazingFeature`)
3. 提交更改 (`git commit -m 'Add some AmazingFeature'`)
4. 推送到分支 (`git push origin feature/AmazingFeature`)
5. 开启 Pull Request

### 代码规范
- 前端：遵循JavaScript Standard Style
- 后端：使用`cargo fmt`和`cargo clippy`
- 提交信息：遵循Conventional Commits规范

## 📄 许可证

本项目采用 [MIT License](LICENSE) 开源协议。

## 🙏 致谢

- [Tauri](https://tauri.app/) - 跨平台桌面应用框架
- [Rust](https://www.rust-lang.org/) - 系统编程语言
- 所有贡献者和用户

## 📞 联系方式

- 项目地址：[GitHub仓库](../../)
- 问题反馈：[Issues](../../issues)
- 邮件联系：[your-email@example.com](mailto:your-email@example.com)

---

⭐ 如果这个项目对你有帮助，请给个Star支持一下！