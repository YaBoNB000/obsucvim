-- WindUI Wind Flow example + safe VX feature migration
-- UI shell/character overlay preserved from the user's example.
-- Migrated: dashboard, read-only players, HUD metrics, console, config/settings.
-- Intentionally excluded: aim/ESP/silent aim, automation, teleport, remote abuse,
-- hidden-property manipulation, noclip/flight and other gameplay cheats.

local MAIN_URL = "https://api.pastes.dev/TKhDKeCrPs"

local Source = game:HttpGet(MAIN_URL)
local LoadWindUI, CompileError = loadstring(Source)
assert(LoadWindUI, "Failed to compile WindUI: " .. tostring(CompileError))

local WindUI = LoadWindUI()
assert(WindUI, "WindUI did not return its API table")

if WindUI.Loader then
    WindUI.Loader:SetStage("Loading migrated interface", 0.9)
end

-- Window shell is intentionally kept the same; only controls/tabs below move.
local Window = WindUI:CreateWindow({
    Title = "Wind Flow Demo",
    Author = "Remote loading example",
    Folder = "WindUI-Demo",
    Icon = "wind",
    Size = UDim2.fromOffset(620, 470),

    Theme = "Dark",
    NewElements = true,
    HideSearchBar = false,
    Acrylic = false,

    OpenButton = {
        Enabled = true,
        Title = "Open WindUI",
        Draggable = true,
        OnlyMobile = false,
        CornerRadius = UDim.new(1, 0),
    },
})

-- Optional monochrome character overlay (preserved from the example).
local TOP_CHARACTER_ASSET_ID = ""
local TOP_CHARACTER_URL = "https://raw.githubusercontent.com/YaBoNB000/bot/main/top-character.png"
local TOP_CHARACTER_FILE = "top-character.png"
local TOP_CHARACTER_PNG_SIGNATURE = "\137PNG\r\n\26\n"

local function topCharacterFileIsValid()
    if type(isfile) ~= "function" or type(readfile) ~= "function" then
        return false
    end

    local Success, Data = pcall(function()
        if not isfile(TOP_CHARACTER_FILE) then
            return nil
        end
        return readfile(TOP_CHARACTER_FILE)
    end)

    return Success
        and type(Data) == "string"
        and Data:sub(1, 8) == TOP_CHARACTER_PNG_SIGNATURE
end

local function resolveTopCharacterImage()
    if TOP_CHARACTER_ASSET_ID ~= "" then
        return TOP_CHARACTER_ASSET_ID
    end

    local AssetResolver = getcustomasset or getsynasset
    if type(AssetResolver) ~= "function" then
        return nil
    end

    if not topCharacterFileIsValid() then
        if type(writefile) ~= "function" then
            return nil
        end

        local DownloadSuccess, Data = pcall(function()
            return game:HttpGet(TOP_CHARACTER_URL)
        end)
        if not DownloadSuccess
            or type(Data) ~= "string"
            or #Data > 8 * 1024 * 1024
            or Data:sub(1, 8) ~= TOP_CHARACTER_PNG_SIGNATURE then
            return nil
        end

        local WriteSuccess = pcall(function()
            writefile(TOP_CHARACTER_FILE, Data)
        end)
        if not WriteSuccess then
            return nil
        end
    end

    local AssetSuccess, Image = pcall(AssetResolver, TOP_CHARACTER_FILE)
    if AssetSuccess and type(Image) == "string" and Image ~= "" then
        return Image
    end

    return nil
end

local TopCharacterImage = resolveTopCharacterImage()
local WindowRoot = Window.UIElements and Window.UIElements.Main

if WindowRoot and TopCharacterImage then
    local TopCharacter = Instance.new("ImageLabel")
    TopCharacter.Name = "TopCharacter"
    TopCharacter.BackgroundTransparency = 1
    TopCharacter.BorderSizePixel = 0
    TopCharacter.Image = TopCharacterImage
    TopCharacter.ScaleType = Enum.ScaleType.Fit
    TopCharacter.AnchorPoint = Vector2.new(0.5, 0)
    TopCharacter.Position = UDim2.new(0.5, 0, -0.31, 0)
    TopCharacter.Size = UDim2.new(0.36, 0, 0.44, 0)
    TopCharacter.ZIndex = 200
    TopCharacter.ImageTransparency = 0
    TopCharacter.Visible = true
    TopCharacter.Active = false
    TopCharacter.Selectable = false
    TopCharacter.Parent = WindowRoot
else
    warn("[WindUI] Top character unavailable; continuing without overlay.")
end

-- Safe runtime shared by migrated controls.
local Players = game:GetService("Players")
local RunService = game:GetService("RunService")
local Stats = game:GetService("Stats")
local LocalPlayer = Players.LocalPlayer
local StartedAt = os.clock()
local Running = true
local Connections = {}
local ConsoleLines = {}
local ConsoleLimit = 80
local ConsoleCode
local ConsoleLevel = "Info"
local ConsoleDraft = ""
local UpdateInterval = 1
local HudUpdates = true
local NotifyJoinLeave = false
local Fps = 0
local Ping = 0
local FrameCount = 0
local FrameTime = 0

local function track(Connection)
    Connections[#Connections + 1] = Connection
    return Connection
end

local function safe(Callback)
    return function(...)
        local Packed = table.pack(pcall(Callback, ...))
        if not Packed[1] then
            warn("[WindUI Safe Port] " .. tostring(Packed[2]))
        end
        return table.unpack(Packed, 2, Packed.n)
    end
end

local function notify(Title, Content, Icon)
    WindUI:Notify({
        Title = Title,
        Content = Content,
        Icon = Icon or "info",
        Duration = 4,
    })
end

local function pushConsole(Kind, ...)
    local Parts = {}
    for Index = 1, select("#", ...) do
        Parts[Index] = tostring(select(Index, ...))
    end
    local Prefix = string.format("[%s] [%s] ", os.date("%H:%M:%S"), Kind:upper())
    ConsoleLines[#ConsoleLines + 1] = Prefix .. table.concat(Parts, "\t")
    while #ConsoleLines > ConsoleLimit do
        table.remove(ConsoleLines, 1)
    end
    if ConsoleCode then
        ConsoleCode:SetCode(table.concat(ConsoleLines, "\n"))
    end
end

local function getPing()
    local Ok, Value = pcall(function()
        return Stats.Network.ServerStatsItem["Data Ping"]:GetValue()
    end)
    return Ok and math.floor(Value + 0.5) or 0
end

local function getCharacterInfo(Player)
    local Character = Player and Player.Character
    local Humanoid = Character and Character:FindFirstChildOfClass("Humanoid")
    local Root = Character and Character:FindFirstChild("HumanoidRootPart")
    local LocalCharacter = LocalPlayer.Character
    local LocalRoot = LocalCharacter and LocalCharacter:FindFirstChild("HumanoidRootPart")
    local Distance
    if Root and LocalRoot then
        Distance = math.floor((Root.Position - LocalRoot.Position).Magnitude + 0.5)
    end
    return Humanoid, Distance
end

-- Config manager is initialized before Flag-bearing controls.
local UserConfig
pcall(function()
    UserConfig = Window.ConfigManager:Config("VX-Safe-Port")
end)

-- Sidebar/tab classification.
local MonitorTabs = Window:Section({
    Title = "Monitor",
    Icon = "activity",
    Opened = true,
})
local UtilityTabs = Window:Section({
    Title = "Utilities",
    Icon = "wrench",
    Opened = true,
})
local SystemTabs = Window:Section({
    Title = "System",
    Icon = "settings",
    Opened = true,
})

local Dashboard = MonitorTabs:Tab({ Title = "Dashboard", Icon = "layout-dashboard" })
local PlayersTab = MonitorTabs:Tab({ Title = "Players", Icon = "users" })
local HudTab = MonitorTabs:Tab({ Title = "HUD", Icon = "chart-no-axes-combined" })
local ConsoleTab = UtilityTabs:Tab({ Title = "Console", Icon = "terminal" })
local SettingsTab = SystemTabs:Tab({ Title = "UI Settings", Icon = "settings-2" })
local AboutTab = SystemTabs:Tab({ Title = "About", Icon = "circle-help" })

local FpsTag = Window:Tag({ Title = "FPS: --", Color = Color3.fromRGB(90, 200, 255) })
local PingTag = Window:Tag({ Title = "Ping: --", Color = Color3.fromRGB(110, 220, 150) })
local PlayerTag = Window:Tag({ Title = "Players: --", Color = Color3.fromRGB(190, 130, 255) })

-- Dashboard (from vx/dashboard.lua: identity, runtime and status cards).
local Welcome = Dashboard:Paragraph({
    Title = "Welcome, " .. LocalPlayer.DisplayName,
    Desc = "@" .. LocalPlayer.Name .. "  •  UserId " .. LocalPlayer.UserId,
    Image = ("rbxthumb://type=AvatarHeadShot&id=%d&w=150&h=150"):format(LocalPlayer.UserId),
    ImageSize = 48,
})

local DashboardStats = Dashboard:Paragraph({
    Title = "Session status",
    Desc = "Collecting FPS, ping and player count...",
    Image = "activity",
    ImageSize = 22,
})

local MemoryProgress = Dashboard:ProgressBar({
    Title = "Client memory",
    Desc = "Approximate Roblox client memory usage",
    Value = { Min = 0, Max = 2000, Default = 0 },
    DisplayMode = "Value",
    Width = 220,
    ValueWidth = 76,
    Format = function(Value)
        return string.format("%.0f MB", Value)
    end,
})

Dashboard:Button({
    Title = "Refresh dashboard",
    Desc = "Refresh safe client-side diagnostics",
    Icon = "refresh-cw",
    Callback = safe(function()
        local Memory = 0
        pcall(function()
            Memory = Stats:GetTotalMemoryUsageMb()
        end)
        MemoryProgress:Set(math.clamp(Memory, 0, 2000))
        DashboardStats:SetDesc(string.format(
            "FPS %d  •  Ping %d ms  •  Players %d  •  Uptime %ds",
            Fps,
            Ping,
            #Players:GetPlayers(),
            math.floor(os.clock() - StartedAt)
        ))
        notify("Dashboard", "Diagnostics refreshed", "circle-check")
    end),
})

Dashboard:Button({
    Title = "Test notification",
    Icon = "bell",
    Callback = function()
        notify("WindUI", "The migrated dashboard is working.", "circle-check")
    end,
})

-- Read-only players page (from vx/players.lua; action/teleport controls excluded).
local PlayerSection = PlayersTab:Section({
    Title = "Player browser",
    Desc = "Read-only information; no spectate, teleport or player manipulation",
    Icon = "users",
    Opened = true,
    Box = true,
})

local PlayerDetails = PlayerSection:Paragraph({
    Title = "No player selected",
    Desc = "Choose a player from the dropdown.",
    Image = "user-search",
    ImageSize = 22,
})

local PlayerDropdown
local PlayerByLabel = {}

local function playerLabel(Player)
    return string.format("%s (@%s) [%d]", Player.DisplayName, Player.Name, Player.UserId)
end

local function selectPlayer(Label)
    local Player = PlayerByLabel[Label]
    if not Player then
        PlayerDetails:SetTitle("Player unavailable")
        PlayerDetails:SetDesc("Refresh the list and try again.")
        return
    end
    local Humanoid, Distance = getCharacterInfo(Player)
    local Team = Player.Team and Player.Team.Name or "No team"
    local Health = Humanoid and string.format("%.0f/%.0f", Humanoid.Health, Humanoid.MaxHealth) or "N/A"
    PlayerDetails:SetTitle(Player.DisplayName .. " (@" .. Player.Name .. ")")
    PlayerDetails:SetDesc(string.format(
        "UserId: %d\nTeam: %s\nHealth: %s\nDistance: %s",
        Player.UserId,
        Team,
        Health,
        Distance and (Distance .. " studs") or "N/A"
    ))
    PlayerDetails:SetImage(
        ("rbxthumb://type=AvatarHeadShot&id=%d&w=150&h=150"):format(Player.UserId),
        48
    )
end

local function rebuildPlayers()
    PlayerByLabel = {}
    local Values = {}
    for _, Player in ipairs(Players:GetPlayers()) do
        if Player ~= LocalPlayer then
            local Label = playerLabel(Player)
            Values[#Values + 1] = Label
            PlayerByLabel[Label] = Player
        end
    end
    table.sort(Values)
    if #Values == 0 then
        Values[1] = "No other players"
    end
    if PlayerDropdown then
        PlayerDropdown:Destroy()
    end
    PlayerDropdown = PlayerSection:Dropdown({
        Title = "Select player",
        Desc = "Current server players",
        Values = Values,
        AllowNone = true,
        SearchBarEnabled = true,
        Callback = selectPlayer,
    })
    PlayerTag:SetTitle("Players: " .. #Players:GetPlayers())
end

PlayerSection:Button({
    Title = "Refresh player list",
    Icon = "refresh-cw",
    Callback = safe(rebuildPlayers),
})
rebuildPlayers()

-- HUD/performance metrics (from vx/hud.lua; gameplay overlays excluded).
local HudMetrics = HudTab:Section({
    Title = "Live metrics",
    Icon = "gauge",
    Opened = true,
})

local FpsProgress = HudMetrics:ProgressBar({
    Title = "FPS",
    Value = { Min = 0, Max = 144, Default = 0 },
    DisplayMode = "Value",
    Width = 210,
})

local PingProgress = HudMetrics:ProgressBar({
    Title = "Ping",
    Value = { Min = 0, Max = 500, Default = 0 },
    DisplayMode = "Value",
    Width = 210,
    Format = function(Value)
        return string.format("%d ms", math.floor(Value + 0.5))
    end,
})

HudMetrics:Toggle({
    Title = "Enable HUD updates",
    Desc = "Update the status tags and progress bars",
    Value = true,
    Flag = "hud_updates",
    Callback = function(Value)
        HudUpdates = Value
    end,
})

HudMetrics:Toggle({
    Title = "Player join/leave notifications",
    Value = false,
    Flag = "join_leave_notifications",
    Callback = function(Value)
        NotifyJoinLeave = Value
    end,
})

HudMetrics:Slider({
    Title = "Update interval",
    Desc = "Seconds between metric refreshes",
    Value = { Min = 0.25, Max = 3, Default = 1 },
    Step = 0.25,
    Flag = "hud_interval",
    Callback = function(Value)
        UpdateInterval = Value
    end,
})

-- Safe console (from vx/console.lua; it only emits print/warn and stores a local log).
local ConsoleControls = ConsoleTab:Section({
    Title = "Output console",
    Desc = "Print-only console; arbitrary Luau execution is intentionally unavailable",
    Icon = "terminal",
    Opened = true,
})

ConsoleControls:Input({
    Title = "Message",
    Type = "Textarea",
    Placeholder = "Type a message to print...",
    Callback = function(Value)
        ConsoleDraft = Value
    end,
})

ConsoleControls:Dropdown({
    Title = "Level",
    Values = { "Info", "Warning", "Error" },
    Value = "Info",
    Callback = function(Value)
        ConsoleLevel = Value
    end,
})

local ConsoleButtons = ConsoleControls:HStack({ AutoSpace = true })
ConsoleButtons:Button({
    Title = "Emit",
    Icon = "play",
    Justify = "Center",
    Callback = function()
        local Message = ConsoleDraft ~= "" and ConsoleDraft or "(empty message)"
        if ConsoleLevel == "Warning" then
            warn(Message)
        elseif ConsoleLevel == "Error" then
            warn("[ERROR] " .. Message)
        else
            print(Message)
        end
        pushConsole(ConsoleLevel, Message)
    end,
})

ConsoleButtons:Button({
    Title = "Clear",
    Icon = "trash-2",
    Justify = "Center",
    Callback = function()
        table.clear(ConsoleLines)
        pushConsole("Info", "Console cleared")
    end,
})

ConsoleCode = ConsoleControls:Code({
    Title = "Output",
    Code = "[READY] Safe console initialized",
    CodeSize = 14,
    CanCopied = true,
    Height = UDim.new(0, 190),
    OnCopy = function()
        notify("Console", "Output copied", "copy-check")
    end,
})
pushConsole("Info", "Safe VX feature port initialized")

-- UI/config settings (from vx/config.lua; WindUI's Flag manager is used).
local Appearance = SettingsTab:Section({
    Title = "Appearance",
    Icon = "palette",
    Opened = true,
})

Appearance:Dropdown({
    Title = "Theme",
    Values = { "Dark", "Light" },
    Value = "Dark",
    Flag = "theme",
    Callback = function(Theme)
        pcall(function()
            WindUI:SetTheme(Theme)
        end)
    end,
})

Appearance:Slider({
    Title = "UI scale",
    Value = { Min = 0.75, Max = 1.25, Default = 1 },
    Step = 0.05,
    Flag = "ui_scale",
    Callback = function(Value)
        pcall(function()
            Window:SetUIScale(Value)
        end)
    end,
})

Appearance:Slider({
    Title = "Background transparency",
    Value = { Min = 0, Max = 0.65, Default = 0 },
    Step = 0.05,
    Flag = "background_transparency",
    Callback = function(Value)
        pcall(function()
            Window:SetBackgroundTransparency(Value)
        end)
    end,
})

Appearance:Toggle({
    Title = "Acrylic",
    Value = false,
    Flag = "acrylic",
    Callback = function(Value)
        pcall(function()
            WindUI:ToggleAcrylic(Value)
        end)
    end,
})

Appearance:Toggle({
    Title = "Lower mobile notifications",
    Value = true,
    Flag = "notification_lower",
    Callback = function(Value)
        WindUI:SetNotificationLower(Value)
    end,
})

local ConfigSection = SettingsTab:Section({
    Title = "Configuration",
    Icon = "save",
    Opened = true,
})

ConfigSection:Button({
    Title = "Save configuration",
    Desc = "Save controls that have a Flag",
    Icon = "save",
    Callback = function()
        if not UserConfig then
            notify("Config", "Config manager is unavailable", "triangle-alert")
            return
        end
        local Ok, Error = pcall(function()
            UserConfig:Save()
        end)
        notify(
            "Config",
            Ok and "Configuration saved" or ("Save failed: " .. tostring(Error)),
            Ok and "circle-check" or "triangle-alert"
        )
    end,
})

ConfigSection:Button({
    Title = "Toggle window",
    Icon = "panel-top",
    Callback = function()
        Window:Toggle()
    end,
})

ConfigSection:Button({
    Title = "Center window",
    Icon = "scan",
    Callback = function()
        Window:SetToTheCenter()
    end,
})

-- About / migration report.
AboutTab:Paragraph({
    Title = "Safe VX → WindUI migration",
    Desc = table.concat({
        "Migrated: dashboard status, read-only player browser, FPS/ping HUD, console and config.",
        "Not migrated: aim, ESP, silent aim, auto jobs, teleport, remote manipulation and hidden properties.",
        "The original Wind Flow window shell and top character overlay are unchanged.",
    }, "\n"),
    Image = "shield-check",
    ImageSize = 24,
})

AboutTab:Code({
    Title = "Print-only command",
    Code = [[print("Hello from the WindUI safe console")]],
    CanCopied = true,
    Height = UDim.new(0, 90),
})

AboutTab:Button({
    Title = "Show migration details",
    Icon = "info",
    Callback = function()
        Window:Dialog({
            Title = "Migration details",
            Icon = "shield-check",
            Content = "The safe/read-only parts of dashboard.lua, players.lua, hud.lua, console.lua and config.lua were reorganized into WindUI tabs. Gameplay cheats were intentionally excluded.",
            Buttons = {
                {
                    Title = "OK",
                    Variant = "Primary",
                },
            },
        }):Show()
    end,
})

-- Runtime update loops.
track(RunService.RenderStepped:Connect(function(Delta)
    FrameCount += 1
    FrameTime += Delta
    if FrameTime >= 0.5 then
        Fps = math.floor(FrameCount / FrameTime + 0.5)
        FrameCount = 0
        FrameTime = 0
    end
end))

track(Players.PlayerAdded:Connect(function(Player)
    rebuildPlayers()
    if NotifyJoinLeave then
        notify("Player joined", Player.DisplayName .. " (@" .. Player.Name .. ")", "user-plus")
    end
end))

track(Players.PlayerRemoving:Connect(function(Player)
    rebuildPlayers()
    if NotifyJoinLeave then
        notify("Player left", Player.DisplayName .. " (@" .. Player.Name .. ")", "user-minus")
    end
end))

task.spawn(function()
    while Running do
        if HudUpdates then
            Ping = getPing()
            FpsTag:SetTitle("FPS: " .. Fps)
            FpsTag:SetColor(Fps >= 50 and Color3.fromRGB(80, 220, 120)
                or Fps >= 30 and Color3.fromRGB(255, 190, 70)
                or Color3.fromRGB(255, 90, 90))
            PingTag:SetTitle("Ping: " .. Ping .. "ms")
            PingTag:SetColor(Ping <= 80 and Color3.fromRGB(80, 220, 120)
                or Ping <= 160 and Color3.fromRGB(255, 190, 70)
                or Color3.fromRGB(255, 90, 90))
            PlayerTag:SetTitle("Players: " .. #Players:GetPlayers())
            FpsProgress:Set(math.clamp(Fps, 0, 144))
            PingProgress:Set(math.clamp(Ping, 0, 500))
        end
        task.wait(UpdateInterval)
    end
end)

pcall(function()
    Window:OnDestroy(function()
        Running = false
        for _, Connection in ipairs(Connections) do
            pcall(function()
                Connection:Disconnect()
            end)
        end
        table.clear(Connections)
    end)
end)

Dashboard:Select()
notify("WindUI", "Safe VX feature migration loaded", "circle-check")
