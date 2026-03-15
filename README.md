# Bengal Language

Statically typed scripting language

## WASM Support

Bengal can be compiled to WebAssembly and run in the browser! See the [wasm-example](wasm-example) directory for a complete example.

### Quick Start with WASM

```bash
# Add WASM target
rustup target add wasm32-unknown-unknown

# Install wasm-pack
cargo install wasm-pack

# Build the WASM example
cd wasm-example
wasm-pack build --target web --release
```

# Auto installation script

Linux:

```bash
bash <(curl -Ls https://raw.githubusercontent.com/Nelonn/bengal/refs/heads/main/install.sh)
```

## Requirements

### Debian/Ubuntu:
```bash
sudo apt install libssl-dev
```

### Fedora
```bash
sudo dnf install openssl-devel
```

## License

MIT
