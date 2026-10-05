#!/bin/bash
clear

fail() {
    echo "❌ HATA: $*" >&2
    if [ -t 0 ]; then read -r -p "Kapatmak için Enter'a basın..."; fi
    exit 1
}

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

BUILD_CMD=()
if command -v cargo-tauri >/dev/null 2>&1; then
    BUILD_CMD=(cargo tauri build)
elif cargo tauri --version >/dev/null 2>&1; then
    BUILD_CMD=(cargo tauri build)
elif command -v bun >/dev/null 2>&1; then
    BUILD_CMD=(bunx @tauri-apps/cli build)
elif command -v npx >/dev/null 2>&1; then
    BUILD_CMD=(npx @tauri-apps/cli build)
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
echo "✅ Kullanılacak derleme komutu: ${BUILD_CMD[*]}"
echo

# Tauri beforeBuildCommand, Bun kullanır; npm kurulumu bunun yerine geçmez.
command -v bun >/dev/null 2>&1 || fail "Bun bulunamadı."

# 2. Frontend bağımlılıklarının kontrolü
if [ ! -d "node_modules" ]; then
    echo "📦 node_modules eksik, bağımlılıklar yükleniyor..."
    bun install --frozen-lockfile || fail "Frontend bağımlılıkları yüklenemedi."
fi

# 2b. Rust bordro çekirdeği değiştiyse tarayıcı (WASM) paketini yeniden üret.
# 'bun run build' WASM tazelik kontrolü yapar; eski paket derlemeyi durdurur.
if command -v wasm-pack >/dev/null 2>&1; then
    echo "🦀 WASM bordro motoru yeniden derleniyor..."
    if command -v bun >/dev/null 2>&1; then
        bun run wasm:build || { echo "❌ HATA: WASM derlemesi başarısız."; read -p "Kapatmak için Enter'a basın..."; exit 1; }
    else
        node scripts/build-wasm.mjs || { echo "❌ HATA: WASM derlemesi başarısız."; read -p "Kapatmak için Enter'a basın..."; exit 1; }
    fi
    echo
else
    echo "⚠️  wasm-pack bulunamadı; mevcut WASM paketi kullanılacak (tazelik kontrolü başarısız olursa 'cargo install wasm-pack' kurun)."
    echo
fi

# 3. Derleme ve paketleme işlemi (Release / Production)
echo "🔨 Uygulama derleniyor ve paketleniyor (Tauri Build)..."
echo "Bu işlem biraz zaman alabilir, lütfen bekleyin..."
echo "--------------------------------------------------"

# Workspace çıktısını ve mimariyi açıkça sabitle. Eski src-tauri/target
# paketleri veya önceki çalıştırmanın çıktıları hiçbir zaman seçilmez.
HOST_TARGET="$(rustc -vV | sed -n 's/^host: //p')"
[ -n "$HOST_TARGET" ] || fail "Rust hedef mimarisi belirlenemedi."
export CARGO_TARGET_DIR="$SCRIPT_DIR/target"
BUNDLE_DIR="$CARGO_TARGET_DIR/$HOST_TARGET/release/bundle"
FOUND_APP="$BUNDLE_DIR/macos/4D Bordro Programı.app"
rm -rf "$BUNDLE_DIR" || fail "Önceki hedef paketleri temizlenemedi."

# Ad-hoc imza yerel macOS çalıştırması içindir; Apple notarizasyonu değildir.
"${BUILD_CMD[@]}" --target "$HOST_TARGET" --config '{"bundle":{"macOS":{"signingIdentity":"-"}}}'
BUILD_STATUS=$?
[ "$BUILD_STATUS" -eq 0 ] || fail "Derleme/paketleme başarısız (Hata Kodu: $BUILD_STATUS). Kurulu sürüm güncellenmedi."
[ -d "$FOUND_APP" ] || fail "Bu çalıştırmada beklenen uygulama paketi üretilmedi: $FOUND_APP"
codesign --verify --deep --strict "$FOUND_APP" || fail "Üretilen uygulamanın imzası doğrulanamadı."

echo "✅ Derleme ve paketleme başarıyla tamamlandı."

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
    rm -rf "$TARGET_DIR" 2>/dev/null || sudo rm -rf "$TARGET_DIR" || fail "Eski kurulum kaldırılamadı."
fi

cp -R "$FOUND_APP" /Applications/ 2>/dev/null || sudo cp -R "$FOUND_APP" /Applications/ || fail "Uygulama /Applications klasörüne kopyalanamadı."

if [ ! -d "$TARGET_DIR" ]; then
    echo "❌ HATA: Uygulama /Applications klasörüne kopyalanamadı!"
    echo
    read -p "Kapatmak için Enter'a basın..."
    exit 1
fi

codesign --verify --deep --strict "$TARGET_DIR" || fail "Kurulan uygulama doğrulanamadı."

# Gatekeeper karantina bayrağını temizle
xattr -cr "$TARGET_DIR" 2>/dev/null || true

# Proje içindeki app/ klasörüne de kopyala
mkdir -p "$SCRIPT_DIR/app" || fail "Proje uygulama klasörü oluşturulamadı."
rm -rf "$SCRIPT_DIR/app/$APP_BASENAME" || fail "Projedeki eski uygulama kaldırılamadı."
cp -R "$FOUND_APP" "$SCRIPT_DIR/app/" || fail "Güncel uygulama proje klasörüne kopyalanamadı."
codesign --verify --deep --strict "$SCRIPT_DIR/app/$APP_BASENAME" || fail "Proje uygulaması doğrulanamadı."
xattr -cr "$SCRIPT_DIR/app/$APP_BASENAME" 2>/dev/null || true

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
