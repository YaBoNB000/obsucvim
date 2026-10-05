# WindUI × VX 安全功能移植说明

## 交付文件

```text
handoff/WindUI_vx_safe_port.luau.lua
```

脚本保留了用户示例中的以下 UI 外壳：

- `MAIN_URL = https://api.pastes.dev/TKhDKeCrPs`
- `CreateWindow` 的标题、作者、尺寸、主题、OpenButton 配置
- 顶部人物图片的下载、PNG 校验、缓存和挂载逻辑

只重新组织了窗口内的控件与回调。

## Tab 分类

### Monitor

- **Dashboard**
  - 用户头像、DisplayName、UserId
  - FPS、Ping、玩家数、运行时间
  - 客户端内存进度条
  - 刷新与通知测试
- **Players**
  - 玩家搜索下拉框
  - DisplayName、用户名、UserId、队伍、血量、距离
  - 只读，不包含传送、观战或玩家操作
- **HUD**
  - FPS / Ping ProgressBar
  - Window Tag 状态
  - 更新频率控制
  - 玩家加入/离开通知

### Utilities

- **Console**
  - 文本输入
  - Info / Warning / Error 等级
  - 仅 `print` / `warn`
  - 本地日志、复制与清空
  - 不提供任意 `loadstring` 执行

### System

- **UI Settings**
  - Dark / Light 主题
  - UI Scale
  - 背景透明度
  - Acrylic
  - 移动端通知下移
  - WindUI ConfigManager 保存
  - 窗口居中/开关
- **About**
  - 移植范围说明
  - 示例代码和 Dialog

## 从 VX 仓库采用的安全功能

- `dashboard.lua`：身份与会话状态展示思路
- `players.lua`：玩家列表、队伍、血量、距离的只读部分
- `hud.lua`：FPS/Ping/玩家数量指标
- `console.lua`：有限日志缓冲、清空与输出等级
- `config.lua`：配置保存思路，改用 WindUI `Flag` + `ConfigManager`

## 未移植的功能

以下模块涉及第三方游戏作弊、客户端绕过或服务器远程滥用，因此没有移植：

- Aim Assist、Auto Shoot、Silent Aim
- ESP、Tracer、目标锁定
- Auto ATM/Golf/Farm/Hack/Restaurant/Taxi 等自动化
- Teleport、Noclip、Flight、Infinite Jump
- Auto Cuff、Anti Arrest、No Damage、Infinite Ammo
- RemoteFunction/RemoteEvent 操作、购买调用
- `sethiddenproperty`、SimulationRadius 修改
- 车辆操控、撞车自动化和网络所有权利用

这些功能没有用空按钮伪装；成品中只出现实际可用的安全控件。

## 验证

```bash
luau-compile handoff/WindUI_vx_safe_port.luau.lua
```

当前 Luau 0.735 静态编译通过。实际 WindUI 渲染仍需在具有 Roblox API、HTTP
和该自定义 WindUI 构建的授权测试环境中验证。
