# nymphalis

A command-line download manager for booru imageboards, manga readers, and Patreon.  
Parallel downloads, terminal progress bars, auto-reads browser sessions.

---

## Installation

### AUR (Arch Linux)

```sh
yay -S nymphalis-bin
```

### Binary

Download the binary for your platform from the [releases page](https://github.com/BlucherSKK/nymphalis/releases), make it executable, and put it in `$PATH`:

```sh
chmod +x nymphalis-linux-amd64
sudo mv nymphalis-linux-amd64 /usr/local/bin/nymphalis
```

Available builds: `linux-amd64` (glibc), `linux-amd64-musl` (static), `macos-arm64`.

---

## Supported services

| Service | Commands |
|---|---|
| **gelbooru.com** | `download`, `search`, `set` |
| **desu.uno** | `download`, `search`, `login` |
| **mangadex.org** | `download`, `search` |
| **patreon.com** | `download`, `show`, `login` |

---

## Usage

```
nymphalis <service> <command> [args...]
```

### gelbooru.com

```sh
# download images by tags into a directory
nymphalis gelbooru.com download ./dir tag1 tag2 tag3

# search tags by keyword
nymphalis gelbooru.com search aeg

# save your API credentials
nymphalis set user_id 11111111
nymphalis set api_key c76c2060...
```

Aliases: `gelbu`, `gel`, `gelbooru`

### desu.uno

```sh
# download all chapters of a manga
nymphalis desu.uno download imaizumins-house-is-a-place-for-gals-to-gather.5467

# search manga by keyword
nymphalis desu.uno search "гал"

# save session from browser (firefox / chromium / chrome / brave / falkon)
nymphalis desu.uno login firefox
```

Alias: `desu`

### mangadex.org

```sh
# download Russian chapters of a manga (by title slug or UUID)
nymphalis mangadex.org download chainsaw-man

# search manga
nymphalis mangadex.org search "chainsaw"
```

Aliases: `mangadex`, `md`, `mdex`

### patreon.com

```sh
# list active subscriptions
nymphalis patreon.com show

# download all media from creators
nymphalis patreon.com download ./dir creator1 creator2

# save session from browser
nymphalis patreon.com login chromium
```

Alias: `patreon`

### Global commands

```sh
# set config variable
nymphalis set <variable> <value>

# proxy for desu.uno / patreon (socks5h supported)
nymphalis set desu_proxy    socks5h://192.168.0.1:1080
nymphalis set patreon_proxy socks5h://192.168.0.1:1080

# parallel download workers (default: 4)
nymphalis set jobs 8

# install shell completions (fish / bash / zsh)
nymphalis add-shell-completions
```

Config is stored in `~/.config/nymphalis.conf`.

---

## Contributing

Fork the repo, make your changes, open a pull request into the `main` branch — that's it.  
There are no strict rules: if the change makes sense and doesn't break existing behaviour, it'll be merged.

---

---

# nymphalis

Консольный менеджер загрузок для бору-имиджбордов, манга-ридеров и Patreon.  
Параллельные загрузки, прогресс-бары в терминале, автоматическое чтение сессий из браузеров.

---

## Установка

### AUR (Arch Linux)

```sh
yay -S nymphalis-bin
```

### Бинарник

Скачай бинарник для своей платформы со [страницы релизов](https://github.com/BlucherSKK/nymphalis/releases), сделай исполняемым и положи в `$PATH`:

```sh
chmod +x nymphalis-linux-amd64
sudo mv nymphalis-linux-amd64 /usr/local/bin/nymphalis
```

Доступные сборки: `linux-amd64` (glibc), `linux-amd64-musl` (статическая), `macos-arm64`.

---

## Поддерживаемые сервисы

| Сервис | Команды |
|---|---|
| **gelbooru.com** | `download`, `search`, `set` |
| **desu.uno** | `download`, `search`, `login` |
| **mangadex.org** | `download`, `search` |
| **patreon.com** | `download`, `show`, `login` |

---

## Использование

```
nymphalis <сервис> <команда> [аргументы...]
```

### gelbooru.com

```sh
# скачать изображения по тегам в директорию
nymphalis gelbooru.com download ./dir tag1 tag2 tag3

# поиск тегов по ключевому слову
nymphalis gelbooru.com search aeg

# сохранить учётные данные API
nymphalis set user_id 1955543
nymphalis set api_key c76c2060...
```

Псевдонимы: `gelbu`, `gel`, `gelbooru`

### desu.uno

```sh
# скачать все главы манги
nymphalis desu.uno download imaizumins-house-is-a-place-for-gals-to-gather.5467

# поиск манги по ключевому слову
nymphalis desu.uno search "гал"

# сохранить сессию из браузера (firefox / chromium / chrome / brave / falkon)
nymphalis desu.uno login firefox
```

Псевдоним: `desu`

### mangadex.org

```sh
# скачать русские главы манги (по slug или UUID)
nymphalis mangadex.org download chainsaw-man

# поиск манги
nymphalis mangadex.org search "chainsaw"
```

Псевдонимы: `mangadex`, `md`, `mdex`

### patreon.com

```sh
# посмотреть активные подписки
nymphalis patreon.com show

# скачать медиа у авторов
nymphalis patreon.com download ./dir creator1 creator2

# сохранить сессию из браузера
nymphalis patreon.com login chromium
```

Псевдоним: `patreon`

### Глобальные команды

```sh
# установить значение конфига
nymphalis set <переменная> <значение>

# прокси для desu.uno / patreon (поддерживается socks5h)
nymphalis set desu_proxy    socks5h://192.168.0.1:1080
nymphalis set patreon_proxy socks5h://192.168.0.1:1080

# количество параллельных воркеров (по умолчанию: 4)
nymphalis set jobs 8

# установить автодополнения для shell (fish / bash / zsh)
nymphalis add-shell-completions
```

Конфиг хранится в `~/.config/nymphalis.conf`.

---

## Контрибьюция

Форкни репозиторий, внеси изменения, открой pull request в ветку `main` — вот и всё.
Строгих правил нет: если изменение имеет смысл и не ломает существующее поведение — смержим.
