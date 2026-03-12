# Velos demo GIF
# Prerequisites:
#   cd examples/rust-binary && cargo build --release && cd ../..
# Run: vhs demo.tape
# Output: demo.gif

Output demo.gif

Set Shell "bash"
Set FontSize 14
Set Width 1200
Set Height 650
Set Theme "Catppuccin Mocha"
Set TypingSpeed 60ms
Set Padding 20

# Clean slate (hidden): kill daemon so all processes stop, then clear
Hide
Type "kill $(cat ~/.velos/velos.pid 2>/dev/null) 2>/dev/null; sleep 1; clear"
Enter
Sleep 3s
Show

# Start Node.js web server
Type "velos start --config examples/node-web-server/velos.toml"
Enter
Sleep 3s

# Start Python FastAPI
Type "velos start --config examples/python-fastapi/velos.toml"
Enter
Sleep 3s

# Start Rust binary service
Type "velos start --config examples/rust-binary/velos.toml"
Enter
Sleep 3s

# Show process list
Type "velos list"
Enter
Sleep 3s

# Show AI-friendly compact output
Type "velos list --ai"
Enter
Sleep 3s

# Smart log summary
Type "velos logs web --summary"
Enter
Sleep 3s

# Open TUI dashboard
Type "velos monit"
Enter
Sleep 5s

Down
Sleep 1s
Down
Sleep 1s
Up
Sleep 1s

Type "q"
Sleep 1s
