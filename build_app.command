#!/bin/bash
clear

# Betiğin bulunduğu dizini otomatik tespit et
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$SCRIPT_DIR" || exit 1

echo "=========================================="
echo "  4/D Bordro Programı - macOS Build & Deploy"
echo "=========================================="
echo
echo "Çalışma Dizini: $SCRIPT_DIR"
echo

# macOS GUI oturumlarında (çift tıklama ile açıldığında) Terminal ortam değişkenlerini yükle
[ -f "$HOME/.zprofile" ] && source "$HOME/.zprofile" >/dev/null 2>&1
[ -f "$HOME/.zshrc" ] && source "$HOME/.zshrc" >/dev/null 2>&1
[ -f "$HOME/.bash_profile" ] && source "$HOME/.bash_profile" >/dev/null 2>&1
[ -f "$HOME/.bashrc" ] && source "$HOME/.bashrc" >/dev/null 2>&1
[ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env" >/dev/null 2>&1

# Olası derleyici ve paket yöneticisi dizinlerini PATH'e ekle
export PATH="/Volumes/Lacie/Developer/Rust/cargo/bin:$HOME/.cargo/bin:$HOME/.bun/bin:/opt/homebrew/bin:/opt/homebrew/sbin:/usr/local/bin:$PATH"

# 1. Gerekli araçların kontrolü
echo "🔍 Geliştirme araçları kontrol ediliyor..."

if ! command -v cargo >/dev/null 2>&1; then
    echo "❌ HATA: 'cargo' bulunamadı!"
    echo "Rust ve Cargo'nun kurulu ve PATH'e eklenmiş olduğundan emin olun."
    echo
    read -p "Kapatmak için Enter'a basın..."
    exit 1
fi

BUILD_CMD=""
if command -v cargo-tauri >/dev/null 2>&1; then
    BUILD_CMD="cargo tauri build"
elif cargo tauri --version >/dev/null 2>&1; then
    BUILD_CMD="cargo tauri build"
elif command -v bun >/dev/null 2>&1; then
    BUILD_CMD="bunx @tauri-apps/cli build"
elif command -v npx >/dev/null 2>&1; then
    BUILD_CMD="npx @tauri-apps/cli build"
else
    echo "❌ HATA: Tauri CLI bulunamadı!"
    echo "Lütfen aşağıdaki yöntemlerden biriyle Tauri CLI yükleyin:"
    echo "  - cargo install tauri-cli --version '^2.0.0'"
    echo "  - bun add -d @tauri-apps/cli@^2.0.0"
    echo
    read -p "Kapatmak için Enter'a basın..."
    exit 1
fi

echo "✅ Rust/Cargo bulundu: $(cargo --version)"
echo "✅ Kullanılacak derleme komutu: $BUILD_CMD"
echo

# 2. Frontend bağımlılıklarının kontrolü
if [ ! -d "node_modules" ]; then
    echo "📦 node_modules eksik, bağımlılıklar yükleniyor..."
    if command -v bun >/dev/null 2>&1; then
        bun install
    elif command -v npm >/dev/null 2>&1; then
        npm install
    fi
    echo
fi

# 3. Derleme ve paketleme işlemi (Release / Production)
echo "🔨 Uygulama derleniyor ve paketleniyor (Tauri Build)..."
echo "Bu işlem biraz zaman alabilir, lütfen bekleyin..."
echo "--------------------------------------------------"

$BUILD_CMD

BUILD_STATUS=$?
echo "--------------------------------------------------"

if [ $BUILD_STATUS -ne 0 ]; then
    echo
    echo "❌ HATA: Uygulama derleme işlemi başarısız oldu (Hata Kodu: $BUILD_STATUS)!"
    echo "Lütfen yukarıdaki hata mesajlarını inceleyin."
    echo
    read -p "Kapatmak için Enter'a basın..."
    exit 1
fi

echo
echo "✅ Derleme ve paketleme başarıyla tamamlandı."
echo

# 4. Üretilen .app paketini tespit et
echo "🔍 Üretilen .app paketi aranıyor..."

FOUND_APP=""
CANDIDATES=(
    "$SCRIPT_DIR/src-tauri/target/release/bundle/macos/4D Bordro Programı.app"
    "$SCRIPT_DIR/src-tauri/target/release/bundle/macos"/*.app
    "$SCRIPT_DIR/src-tauri/target/aarch64-apple-darwin/release/bundle/macos/4D Bordro Programı.app"
    "$SCRIPT_DIR/src-tauri/target/aarch64-apple-darwin/release/bundle/macos"/*.app
    "$SCRIPT_DIR/src-tauri/target/x86_64-apple-darwin/release/bundle/macos/4D Bordro Programı.app"
    "$SCRIPT_DIR/src-tauri/target/x86_64-apple-darwin/release/bundle/macos"/*.app
    "$SCRIPT_DIR/target/release/bundle/macos"/*.app
)

for candidate in "${CANDIDATES[@]}"; do
    if [ -d "$candidate" ]; then
        FOUND_APP="$candidate"
        break
    fi
done

if [ -z "$FOUND_APP" ] || [ ! -d "$FOUND_APP" ]; then
    FOUND_APP=$(find "$SCRIPT_DIR/src-tauri/target" -type d -name "*.app" -path "*/bundle/macos/*" 2>/dev/null | head -n 1)
fi

if [ -z "$FOUND_APP" ] || [ ! -d "$FOUND_APP" ]; then
    echo "❌ HATA: Derlenen .app paketi 'target/.../bundle/macos/' altında bulunamadı!"
    echo
    read -p "Kapatmak için Enter'a basın..."
    exit 1
fi

APP_BASENAME="$(basename "$FOUND_APP")"
TARGET_DIR="/Applications/$APP_BASENAME"

echo "📦 Bulunan paket: $FOUND_APP"
echo "🎯 Hedef: $TARGET_DIR"
echo

# 5. Eğer uygulama şu anda çalışıyorsa kapat
if pgrep -fi "$APP_BASENAME" >/dev/null 2>&1 || pgrep -f "bordro-programi" >/dev/null 2>&1; then
    echo "⚠️  Uygulama şu anda açık görünüyor. Güncelleme için kapatılıyor..."
    pkill -fi "$APP_BASENAME" 2>/dev/null || true
    pkill -f "bordro-programi" 2>/dev/null || true
    sleep 1
fi

# 6. /Applications klasörüne yükle
echo "📂 Uygulama /Applications klasörüne kopyalanıyor..."

if [ -d "$TARGET_DIR" ]; then
    echo "Eski sürüm kaldırılıyor..."
    rm -rf "$TARGET_DIR" 2>/dev/null || sudo rm -rf "$TARGET_DIR"
fi

cp -R "$FOUND_APP" /Applications/ 2>/dev/null || sudo cp -R "$FOUND_APP" /Applications/

if [ ! -d "$TARGET_DIR" ]; then
    echo "❌ HATA: Uygulama /Applications klasörüne kopyalanamadı!"
    echo
    read -p "Kapatmak için Enter'a basın..."
    exit 1
fi

# Gatekeeper karantina bayrağını temizle
xattr -cr "$TARGET_DIR" 2>/dev/null || true

echo
echo "=========================================="
echo "🎉 Kurulum Başarıyla Tamamlandı!"
echo "=========================================="
echo "Uygulama başarıyla yüklendi: $TARGET_DIR"
echo

# 7. Çalıştırma onayı
read -p "Uygulamayı şimdi başlatmak ister misiniz? [E/h]: " RUN_CONFIRM
case "$RUN_CONFIRM" in
    h|H|hayır|HAYIR|n|N|no|NO)
        echo "Uygulama başlatılmadı."
        ;;
    *)
        echo "🚀 Uygulama başlatılıyor..."
        open "$TARGET_DIR"
        ;;
esac

echo
read -p "Kapatmak için Enter'a basın..."
