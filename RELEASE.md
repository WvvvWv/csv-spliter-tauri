# 发布指南

## 如何创建新版本发布

### 1. 创建新版本标签
```bash
git tag v0.1.0
git push origin v0.1.0
```

### 2. GitHub Actions 会自动执行以下操作：
- 在多个平台（Windows、macOS、Linux）上构建应用
- 创建GitHub Release
- 上传构建好的安装包
- 更新latest.json文件用于自动更新

### 3. 发布完成后
- 在GitHub Releases页面查看发布结果
- 下载对应平台的安装包
- 验证安装包是否可以正常安装和运行

## 构建目标

### Windows
- 构建输出：`.msi`安装包
- 支持：Windows 10/11 64位

### macOS
- 构建输出：`.dmg`安装包
- 支持：Intel和Apple Silicon (M1/M2)

### Linux
- 构建输出：`.AppImage`
- 支持：大多数Linux发行版

## 注意事项

1. 确保在发布前所有测试都通过
2. 更新版本号在以下文件中：
   - `src-tauri/Cargo.toml`
   - `src-tauri/tauri.conf.json`
   - `package.json`

3. 发布标签必须使用语义化版本格式：v1.0.0, v1.0.1等