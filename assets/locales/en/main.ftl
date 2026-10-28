## Title bar and menu
app-name = Capture Studio
menu-file = File
menu-scenes = Scenes
menu-view = View
menu-settings = Settings
menu-file-profiles = Profiles
menu-file-collections = Scene collections
menu-file-open-recordings = Open recordings folder
menu-file-exit = Exit
menu-scenes-add = Add
menu-scenes-duplicate = Duplicate
menu-scenes-remove = Remove
menu-scenes-transition = Transition
transition-cut = Cut
transition-fade = Fade
menu-view-theme = Theme
menu-view-mini = Mini mode
menu-view-mixer = Mixer
menu-view-stats = Statistics
menu-view-hide-preview = Hide preview
menu-view-show-preview = Show preview
menu-soon = soon
window-minimize = Minimize
window-maximize = Maximize
window-restore = Restore
window-close = Close

indicator-live = LIVE
indicator-rec = REC

## Left panel
section-scenes = SCENES
section-sources = SOURCES
section-audio = AUDIO
scene-new-name = Scene { $n }
scene-rename = Rename
source-visible = Hide source
source-hidden = Show source
meta-browser = Browser · { $w }×{ $h }
meta-monitor = Display { $n } · { $w }×{ $h }
meta-color = Color · { $w }×{ $h }
meta-text = Text
meta-method-hook = hook { $api }
meta-method-wgc = WGC
meta-minimized = window minimized
add-source-soon = Adding sources arrives in M1

## Preview
preview-program = PROGRAM
preview-placeholder = scene preview · { $w }×{ $h } · { $fps } fps
param-canvas = Canvas { $w }×{ $h }
param-output = Output { $w }×{ $h }
param-bitrate = { $kbps } kbps { $rc }

## Control buttons
btn-start-stream = Start streaming
btn-stop-stream = Stop streaming
btn-connecting = Connecting…
btn-confirm-stop = Click again to stop
btn-stopping = Stopping…
btn-start-record = Start recording
btn-stop-record = Stop recording

## Status bar
status-cpu = CPU
status-gpu = GPU
status-fps = FPS
status-bitrate = Bitrate
status-drops = Dropped
status-rec = REC
status-connection-ok = ● Connection stable
status-reconnecting = ● Reconnecting ({ $n }/{ $max })
status-no-connection = ● No connection
unit-kbps = kbps

## Settings (1c)
settings-tab-general = General
settings-tab-stream = Stream
settings-tab-output = Output
settings-tab-audio = Audio
settings-tab-video = Video
settings-tab-hotkeys = Hotkeys
settings-cancel = Cancel
settings-apply = Apply
settings-locked-live = Stop the stream to change this
settings-advanced = Advanced
settings-estimate = Estimate: ~{ $gb } GB/hour of recording · GPU load ≈ { $gpu }%
settings-estimate-cq = Estimate: ≈{ $gb } GB/hour of recording · GPU load ≈ { $gpu }%
settings-saved = Settings saved
settings-save-failed = Failed to save settings: { $err }

field-encoder = Encoder
field-rate-control = Rate control
field-bitrate = Bitrate
field-keyint = Keyframe interval
field-preset = Preset
field-record-format = Recording format
field-profile = Profile
field-bframes = B-frames
field-lookahead = Lookahead
field-separate-encoder = Separate recording encoder
field-tracks = Recording tracks
unit-seconds = s
unit-nits = nits

encoder-auto = Auto (best available)
encoder-nvenc = NVIDIA NVENC H.264
encoder-amf = AMD AMF H.264
encoder-qsv = Intel QSV H.264
encoder-mf = Media Foundation H.264
encoder-software = Software H.264 (OpenH264)
preset-1 = P1 · Fastest
preset-2 = P2 · Faster
preset-3 = P3 · Fast
preset-4 = P4 · Medium
preset-5 = P5 · Slow
preset-6 = P6 · Slower
preset-7 = P7 · Slowest
record-mkv-remux = MKV → auto-remux to MP4
record-mkv = MKV
record-mp4 = MP4
record-fmp4 = Fragmented MP4

field-language = Language
field-theme = Theme
field-start-minimized = Start minimized
field-confirm-stop = Confirm stop
field-check-updates = Check for updates
field-priority = Process priority
field-recordings-dir = Recordings folder
lang-ru = Русский
lang-en = English
theme-dark = Dark
theme-light = Light
theme-system = System
priority-normal = Normal
priority-above-normal = Above normal
priority-high = High
yes = Yes
no = No
default-recordings-dir = Videos\Capture Studio

field-service = Service
field-server = Server
field-stream-key = Stream key
field-reconnect = Reconnect
field-dynamic-bitrate = Dynamic bitrate
service-custom = Custom
server-auto = Auto (nearest)
key-show = Show
key-hide = Hide
key-not-set = not set

field-sample-rate = Sample rate
field-desktop-device = Desktop audio
field-mic-device = Microphone
field-monitor-device = Monitoring
device-default = Default

field-canvas = Canvas resolution
field-output-res = Output resolution
field-scale-filter = Scale filter
field-fps = Frame rate
field-color-space = Color space
field-color-range = Range
field-sdr-white = SDR white level
filter-bilinear = Bilinear
filter-bicubic = Bicubic
filter-lanczos = Lanczos
range-limited = Limited
range-full = Full
res-monitor = Match monitor

hotkeys-search = Search…
hotkeys-reset = Reset
hotkeys-press = Press a combination…
hotkeys-none = —
hotkeys-conflict = Already assigned: { $action }
hotkey-action-scene = Scene { $n }
hotkey-action-stream = Start/stop streaming
hotkey-action-record = Start/stop recording
hotkey-action-pause = Pause recording
hotkey-action-mute-mic = Mute microphone
hotkey-action-ptt = Push-to-talk
hotkey-action-mini = Mini mode

## Mixer (1d)
mixer-title = MIXER
mixer-mute = M
mixer-filters = Filters
mixer-monitor = Monitoring
mixer-monitor-off = Off
mixer-monitor-only = Monitor only
mixer-monitor-output = Monitor and output
mixer-tracks = Tracks
mixer-filters-soon = Filter chain arrives in M2

## Statistics (1e)
stats-title = STATISTICS
stats-uptime = Time live
stats-bitrate = Bitrate
stats-dropped = Dropped frames
stats-render = Render
stats-rtt = Network latency
stats-transferred = Sent
stats-chart = BITRATE · 10 MIN
stats-chart-tip = { $kbps } kbps · −{ $ago }
stats-details = Details
stats-drops-breakdown = Drops: render { $r } · encoder { $e } · network { $n }
stats-encoder-load = Encoder load { $p }%
stats-copy = Copy report
stats-copied = Report copied to clipboard

## Mini mode (1f)
mini-expand = Back to full window
mini-close = Close mini mode
mini-stop-hint = Double-click or hold to stop streaming
mini-drop = { $p }% drop

## Toasts
toast-stream-started = Stream started
toast-stream-stopped = Stream stopped
toast-reconnecting = Reconnecting ({ $n }/{ $max })…
toast-record-started = Recording started
toast-record-stopped = Recording saved: { $path }
toast-remux-done = Remux finished: { $path }
toast-error = Error: { $msg }
toast-tray-soon = Tray mode arrives in M3 — window minimized
