use std::env;
use std::sync::LazyLock;

#[derive(Clone, Copy)]
pub enum Lang {
    En,
    Ru,
}

pub static LANG: LazyLock<Lang> = LazyLock::new(detect_lang);

fn detect_lang() -> Lang {
    for var in ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"] {
        if let Ok(val) = env::var(var) {
            if val.to_lowercase().starts_with("ru") {
                return Lang::Ru;
            }
        }
    }
    Lang::En
}

const STRINGS: &[(&str, &str)] = &[
    // Usage
    ("Usage:", "Использование:"),
    (
        "  nymphalis <service> <command> [args...]",
        "  nymphalis <сервис> <команда> [аргументы...]",
    ),
    ("Services:", "Сервисы:"),
    (
        "  gelbooru.com  (aliases: gelbu, gel, gelbooru)",
        "  gelbooru.com  (псевдонимы: gelbu, gel, gelbooru)",
    ),
    (
        "  desu.uno      (aliases: desu)",
        "  desu.uno      (псевдонимы: desu)",
    ),
    ("Commands (gelbooru.com):", "Команды (gelbooru.com):"),
    (
        "  nymphalis gelbooru.com download <dir> <tag1> [tag2] ...",
        "  nymphalis gelbooru.com download <папка> <тег1> [тег2] ...",
    ),
    (
        "      Download all images for the given tags into <dir>.",
        "      Скачать все изображения по тегам в <папка>.",
    ),
    (
        "  nymphalis gelbooru.com search <keyword>",
        "  nymphalis gelbooru.com search <ключевое_слово>",
    ),
    (
        "      Search for tags matching <keyword>.",
        "      Поиск тегов по <ключевое_слово>.",
    ),
    ("Commands (desu.uno):", "Команды (desu.uno):"),
    (
        "  nymphalis desu.uno download <dir> <slug.id> [slug.id2] ...",
        "  nymphalis desu.uno download <папка> <slug.id> [slug.id2] ...",
    ),
    (
        "      Download all chapters of the given manga title(s) into <dir>.",
        "      Скачать все главы указанной манги в <папка> (можно указать несколько).",
    ),
    (
        "  nymphalis desu.uno search <keyword>",
        "  nymphalis desu.uno search <ключевое_слово>",
    ),
    (
        "      Search for manga. Output: Human Title | slug.id",
        "      Поиск манги. Вывод: Человеческое название | slug.id",
    ),
    ("Global commands:", "Глобальные команды:"),
    (
        "  nymphalis set <variable> <value>",
        "  nymphalis set <variable> <value>",
    ),
    (
        "      Save a setting into {} . Variables: user_id, api_key, jobs.",
        "      Сохранить настройку в {} . Переменные: user_id, api_key, jobs.",
    ),
    ("Examples:", "Примеры:"),
    (
        "  nymphalis set user_id 1955543",
        "  nymphalis set user_id 1955543",
    ),
    (
        "  nymphalis set api_key c76c2060...your_key...",
        "  nymphalis set api_key c76c2060...your_key...",
    ),
    ("  nymphalis set jobs 8", "  nymphalis set jobs 8"),
    (
        "  nymphalis gelbooru.com search aeg",
        "  nymphalis gelbooru.com search aeg",
    ),
    (
        "  nymphalis gelbu download ./downloads cat blue_eyes",
        "  nymphalis gelbu download ./downloads cat blue_eyes",
    ),
    (
        "  nymphalis desu.uno search gals",
        "  nymphalis desu.uno search gals",
    ),
    (
        "  nymphalis desu download ./manga imaizumins-house-is-a-place-for-gals-to-gather.5467",
        "  nymphalis desu download ./manga imaizumins-house-is-a-place-for-gals-to-gather.5467",
    ),
    // Errors
    ("Unknown service: {}\n", "Неизвестный сервис: {}\n"),
    ("Unknown option: {}\n", "Неизвестная опция: {}\n"),
    (
        "Unknown command for {}: {}\n",
        "Неизвестная команда для {}: {}\n",
    ),
    (
        "Failed to build HTTP client: {}",
        "Не удалось создать HTTP-клиент: {}",
    ),
    (
        "Failed to serialize config: {}",
        "Не удалось сериализовать конфиг: {}",
    ),
    ("Failed to write {}: {}", "Не удалось записать {}: {}"),
    (
        "set requires exactly <variable> <value>.\n",
        "set требует ровно <variable> <value>.\n",
    ),
    (
        "Unknown variable '{}'. Supported: user_id, api_key, jobs",
        "Неизвестная переменная '{}'. Поддерживаются: user_id, api_key, jobs",
    ),
    (
        "jobs must be a positive integer.",
        "jobs должно быть положительным целым числом.",
    ),
    ("Saved '{}' to {}", "Сохранено '{}' в {}"),
    (
        "Failed to create directory '{}': {}",
        "Не удалось создать папку '{}': {}",
    ),
    // Gelbooru credentials
    (
        "Warning: GELBOORU_USER_ID / GELBOORU_API_KEY are not set",
        "Внимание: GELBOORU_USER_ID / GELBOORU_API_KEY не заданы",
    ),
    (
        "(checked environment, .env, and {}).",
        "(проверены переменные окружения, .env и {}).",
    ),
    (
        "Gelbooru now rejects anonymous API requests with 401 Unauthorized.",
        "Gelbooru теперь отклоняет анонимные запросы с кодом 401.",
    ),
    (
        "Get your credentials at gelbooru.com -> My Account -> Options ->",
        "Получи учётные данные на gelbooru.com -> My Account -> Options ->",
    ),
    (
        "API Access Credentials, then either:",
        "API Access Credentials, затем либо:",
    ),
    (
        "  nymphalis set user_id 12345",
        "  nymphalis set user_id 12345",
    ),
    (
        "  nymphalis set api_key yourkey",
        "  nymphalis set api_key yourkey",
    ),
    (
        "or export them as environment variables.",
        "либо экспортируй их как переменные окружения.",
    ),
    (
        "401 Unauthorized: Gelbooru requires API credentials for every",
        "401 Unauthorized: Gelbooru требует учётные данные для каждого",
    ),
    (
        "request (anonymous access was disabled in 2025). Set:",
        "запроса (анонимный доступ отключён с 2025 года). Задай:",
    ),
    (
        "  GELBOORU_USER_ID  and  GELBOORU_API_KEY",
        "  GELBOORU_USER_ID  и  GELBOORU_API_KEY",
    ),
    (
        "(get them at gelbooru.com -> My Account -> Options ->",
        "(получи их на gelbooru.com -> My Account -> Options ->",
    ),
    (
        "API Access Credentials) and try again.",
        "API Access Credentials) и попробуй снова.",
    ),
    // Gelbooru download/search
    (
        "download requires a directory and at least one tag.\n",
        "download требует папку и хотя бы один тег.\n",
    ),
    ("Tags: {}", "Теги: {}"),
    ("Output directory: {}", "Папка для сохранения: {}"),
    (
        "API request error (page {}): {}",
        "Ошибка запроса к API (страница {}): {}",
    ),
    (
        "API returned status {} on page {}",
        "API вернул статус {} на странице {}",
    ),
    (
        "No images found for the given tags.",
        "По указанным тегам ничего не найдено.",
    ),
    (
        "No new images to download, everything is already on disk.",
        "Новых изображений нет, всё уже скачано.",
    ),
    ("Checking file sizes...", "Проверка размеров файлов..."),
    ("Parallel workers: {}", "Параллельных потоков: {}"),
    (
        "About to download {} images (~{}). Continue? [y/N]: ",
        "Будет скачано {} изображений (~{}). Продолжить? [y/N]: ",
    ),
    ("Cancelled.", "Отменено."),
    (
        "Done. Downloaded: {}, skipped (already existed): {}",
        "Готово. Скачано: {}, пропущено (уже было): {}",
    ),
    (
        "search expects exactly one keyword.\n",
        "search требует ровно одно ключевое слово.\n",
    ),
    ("API request error: {}", "Ошибка запроса к API: {}"),
    ("API returned status {}", "API вернул статус {}"),
    ("Failed to read response: {}", "Не удалось прочитать ответ: {}"),
    (
        "Failed to parse JSON: {}\nServer response: {}",
        "Не удалось распарсить JSON: {}\nОтвет сервера: {}",
    ),
    (
        "No tags found matching '{}'.",
        "Не найдено тегов, соответствующих '{}'.",
    ),
    ("Tags matching '{}':\n", "Теги, соответствующие '{}':\n"),
    ("TAG", "ТЕГ"),
    ("POSTS", "ПОСТОВ"),
    (
        "{} tags found. Use the exact tag name with: nymphalis gelbooru.com download <dir> <tag>",
        "Найдено тегов: {}. Используй точное имя с: nymphalis gelbooru.com download <папка> <тег>",
    ),
    ("Total", "Всего"),
    ("{} ({}/s)", "{} ({}/с)"),
    ("request error: {}", "ошибка запроса: {}"),
    ("status {}", "статус {}"),
    ("create file: {}", "создание файла: {}"),
    ("read: {}", "чтение: {}"),
    ("write: {}", "запись: {}"),
    ("rename file: {}", "переименование файла: {}"),
    // Desu.uno
    (
        "download requires at least one manga title.\n",
        "download требует хотя бы одно название манги.\n",
    ),
    (
        "No manga found matching '{}'.",
        "Манга по запросу '{}' не найдена.",
    ),
    ("Manga matching '{}':\n", "Манга по запросу '{}':\n"),
    ("HUMAN TITLE", "НАЗВАНИЕ"),
    ("SYSTEM ID", "СИСТЕМНЫЙ ID"),
    (
        "{} results. Use the id/slug.id with: nymphalis desu.uno download <slug.id>",
        "Найдено: {}. Используй id/slug.id с: nymphalis desu.uno download <slug.id>",
    ),
    (
        "Invalid manga id format: '{}'. Expected numeric id or 'slug.id'.",
        "Неверный формат id манги: '{}'. Ожидается числовой id или 'slug.id'.",
    ),
    (
        "Failed to fetch manga info for id {}: {}",
        "Не удалось получить информацию о манге с id {}: {}",
    ),
    (
        "No chapters found for '{}'.",
        "Главы для '{}' не найдены.",
    ),
    (
        "Downloading '{}': {} chapter(s)",
        "Скачивание '{}': {} глав(ы)",
    ),
    (
        "About to download {} chapter(s). Continue? [y/N]: ",
        "Будет скачано {} глав(ы). Продолжить? [y/N]: ",
    ),
    (
        "Failed to fetch pages for chapter {}: {}",
        "Не удалось получить страницы для главы {}: {}",
    ),
    (
        "No pages in chapter {}.",
        "Страниц в главе {} не найдено.",
    ),
    (
        "Chapter {}/{}",
        "Глава {}/{}",
    ),
    (
        "Done. {} chapter(s) downloaded.",
        "Готово. Скачано {} глав(ы).",
    ),
    (
        "Failed to fetch chapters for '{}': {}",
        "Не удалось получить главы для '{}': {}",
    ),
];

pub fn rep(en: &str) -> String {
    match *LANG {
        Lang::En => en.to_string(),
        Lang::Ru => STRINGS
            .iter()
            .find(|(e, _)| *e == en)
            .map(|(_, r)| r.to_string())
            .unwrap_or_else(|| en.to_string()),
    }
}

pub fn repf(en: &str, args: &[&dyn std::fmt::Display]) -> String {
    let mut s = rep(en);
    for a in args {
        if let Some(pos) = s.find("{}") {
            s.replace_range(pos..pos + 2, &a.to_string());
        }
    }
    s
}
