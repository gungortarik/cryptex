CRYPtex VM KONTROL

1. ZIP dosyasini VM'ye kopyalayin ve Tumunu Ayikla ile acin.
2. VM-KONTROL.cmd dosyasina cift tiklayin. Yonetici olarak acmaniz gerekmez.
3. Acilan klasorde secili SONUC.zip dosyasini sohbete ekleyin.

Cryptex acik veya kapali olabilir. Terminale komut yapistirmaniz gerekmez.
Sonuc: %LOCALAPPDATA%\Cryptex-Dev\vm-checks altinda tutulur.

BU PAKET NE YAPAR?
Windows/PowerShell surumunu, yerel IPv4 adreslerini, ilgili surecleri ve
calisan EXE'lerin SHA256 degerlerini toplar. TEMP altindaki isaretli cx-m8
pilot profillerinin eslesme durumunu ve olay dosyasi sayisini okur.
Eski profiller bulunamazsa rapor bunu gosterir; yeni profil olusturmaz.

BU PAKET NEYI KANITLAMAZ?
Cihazlar arasi aktarim, imza dogrulamasi veya M8 tamamlanmasi degildir.
Normal aktarim/yayinlama bu paketle acilmaz. Bu, sonraki aktarim testi
icin tek seferde ortam bilgisini toplama adimidir.

Notlar, olay icerikleri, sifreler, anahtarlar, tam gunlukler ve imzali
mesajlar pakete eklenmez. Rapor yerel IP adreslerini, islem kimliklerini,
pilot klasor etiketlerini ve dosya hash'lerini icerir. Otomatik gonderilmez.
Uygulama/profil/ag/antivirus ayarlari degistirilmez; surecler kapatilmaz.
Baslatici sadece kendi PowerShell oturumunda ExecutionPolicy Bypass kullanir;
kalici Windows ilkesini degistirmez. Kurumsal guvenlik engeli varsa durun.
