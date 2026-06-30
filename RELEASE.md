# Nymphalis — первый публичный релиз

Консольный менеджер загрузок для бору-сайтов, манга-ридеров и Patreon.  
Параллельные загрузки, локализация (ru / en / de), автоматическое чтение сессий из браузеров.

## Поддерживаемые сервисы

| Сервис | Команды |
|---|---|
| **gelbooru.com** | `download`, `search`, `set` |
| **desu.uno** | `download`, `search`, `login` |
| **mangadex.org** | `download`, `search` |
| **patreon.com** | `download`, `show`, `login` |

## Что реализовано

- [x] Скачивание изображений с gelbooru.com по тегам — параллельно, N воркеров
- [x] Поиск тегов на gelbooru.com
- [x] Скачивание манги с desu.uno по главам (с прогресс-баром по страницам)
- [x] Поиск манги на desu.uno
- [x] Авторизация на desu.uno через куки браузера (firefox / chromium / chrome / brave / falkon)
- [x] Скачивание манги с mangadex.org — только русские главы
- [x] Поиск манги на mangadex.org
- [x] Скачивание медиа с Patreon (изображения, вложения, аудио)
- [x] Просмотр активных подписок Patreon (`show`)
- [x] Авторизация на Patreon через куки браузера
- [x] Иерархический прогресс-бар в терминале (indicatif)
- [x] Локализация: автодетект языка через `LC_ALL` / `LANG`, поддержка ru / en / de
- [x] Конфиг-файл (`~/.config/nymphalis.conf`), команда `set`
- [x] Поддержка socks5h-прокси для desu.uno и Patreon
- [x] CI/CD: сборки под linux-amd64 (glibc), linux-amd64 (musl), macos-arm64

## Установка

Скачай бинарник для своей платформы ниже и положи в `$PATH`:

```sh
chmod +x nymphalis-linux-amd64
sudo mv nymphalis-linux-amd64 /usr/local/bin/nymphalis
```

Для статически слинкованной версии без зависимости от системного glibc используй `nymphalis-linux-amd64-musl`.
