# Linux kurulumu

Linux paketi GitHub Actions üzerinden x86_64 mimarisi için üretilir. Bu
değişiklikler GitHub'a gönderildikten sonra `Linux Packages` iş akışını
**Actions → Run workflow** ile elle çalıştırın; paketleri iş akışı
çalışmasının **Artifacts** bölümünden indirebilirsiniz. Her çalıştırma bir
`.deb` ve bir `.AppImage` üretir.

## Debian ve Ubuntu

İndirilen `.deb` dosyasını açıp yazılım kurucusuyla yükleyin veya indirdiğiniz
paket adını kullanarak terminalden:

```bash
sudo apt install ./*.deb
```

`apt`, Tauri paketinin bildirdiği sistem bağımlılıklarını kurar.

## Diğer dağıtımlar

AppImage dosyasını çalıştırılabilir yapıp başlatın:

```bash
chmod +x ./*.AppImage
./*.AppImage
```

AppImage, Ubuntu 22.04 tabanında x86_64 için derlenir. Dağıtımın glibc sürümü
ve masaüstü kitaplıkları nedeniyle eski veya farklı Linux sistemlerinde
çalışacağı garanti edilmez. ARM tabanlı bilgisayarlara yönelik paket bu
iş akışında üretilmez.

Uygulama veritabanı yerel kullanıcı uygulama dizininde tutulur; paketi kaldırmak
kullanıcının veritabanını silmez.
