use indicatif::{HumanBytes, MultiProgress, ProgressBar, ProgressStyle};

// в чём измеряется прогресс одного ContentUnit
#[derive(Clone)]
pub enum ProgressUnit {
    // файл — размер из заголовков, если нет → спиннер
    Bytes,
    // дискретные штуки с известным итогом (страницы, треки…)
    Pages(u64),
}

// одна строка в дисплее загрузки — файл, глава, что угодно
pub struct ContentUnit {
    pub label: String,
    pub unit: ProgressUnit,
}

impl ContentUnit {
    pub fn bytes(label: impl Into<String>) -> Self {
        Self { label: label.into(), unit: ProgressUnit::Bytes }
    }
    pub fn pages(label: impl Into<String>, total: u64) -> Self {
        Self { label: label.into(), unit: ProgressUnit::Pages(total) }
    }
}

// многоуровневая консольная метрика: строки готового сверху, итоговый бар снизу
pub struct DownloadDisplay {
    multi: MultiProgress,
    overall: ProgressBar,
}

// хэндл активного элемента, возвращается из begin()
pub struct ItemHandle {
    bar: ProgressBar,
    label: String,
    unit: ProgressUnit,
    spinner_style: ProgressStyle,
}

impl DownloadDisplay {
    pub fn new(total: u64, prefix: impl Into<String>) -> Self {
        let multi = MultiProgress::new();
        let overall = multi.add(ProgressBar::new(total));
        overall.set_style(
            ProgressStyle::with_template(
                "{prefix:.bold} [{bar:30.cyan/black}] {pos}/{len}  {msg}",
            )
            .unwrap()
            .progress_chars("█▓░"),
        );
        overall.set_prefix(prefix.into().chars().take(22).collect::<String>());
        Self { multi, overall }
    }

    // начать новый элемент — бар вставляется над итоговым
    pub fn begin(&self, unit: ContentUnit) -> ItemHandle {
        let bar = self.multi.insert_before(&self.overall, ProgressBar::new(0));

        let spinner_style = ProgressStyle::with_template(
            "  {msg:<30.green} {spinner:.green} {bytes} ({binary_bytes_per_sec})",
        )
        .unwrap();

        match &unit.unit {
            ProgressUnit::Bytes => {
                bar.set_style(
                    ProgressStyle::with_template(
                        "  {msg:<30.green} [{bar:26.green/black}] \
                         {bytes}/{total_bytes} {binary_bytes_per_sec}",
                    )
                    .unwrap()
                    .progress_chars("#>-"),
                );
            }
            ProgressUnit::Pages(total) => {
                bar.set_length(*total);
                bar.set_style(
                    ProgressStyle::with_template(
                        "  {msg:<32} [{bar:26.green/black}] {pos}/{len} стр.",
                    )
                    .unwrap()
                    .progress_chars("#>-"),
                );
            }
        }

        bar.set_message(unit.label.chars().take(30).collect::<String>());

        ItemHandle { bar, label: unit.label, unit: unit.unit, spinner_style }
    }

    // элемент готов — печатает ✓ и убирает бар
    pub fn end_ok(&self, handle: ItemHandle, amount: u64) {
        let detail = match handle.unit {
            ProgressUnit::Bytes   => format!("{}", HumanBytes(amount)),
            ProgressUnit::Pages(n) => format!("{} стр.", n),
        };
        handle.bar.finish_and_clear();
        let label = handle.label.chars().take(36).collect::<String>();
        let _ = self.multi.println(format!("  \x1b[32m✓\x1b[0m {:<38} {}", label, detail));
    }

    // элемент с ошибкой — печатает ✗ и убирает бар
    pub fn end_err(&self, handle: ItemHandle, err: &str) {
        handle.bar.finish_and_clear();
        let label = handle.label.chars().take(36).collect::<String>();
        let _ = self.multi.println(format!("  \x1b[31m✗\x1b[0m {:<38} {}", label, err));
    }

    pub fn set_msg(&self, msg: impl Into<String>) {
        self.overall.set_message(msg.into());
    }

    pub fn advance(&self) {
        self.overall.inc(1);
    }

    pub fn println(&self, msg: impl AsRef<str>) {
        let _ = self.multi.println(msg.as_ref());
    }

    pub fn finish(&self, msg: impl Into<String>) {
        self.overall.finish_with_message(msg.into());
    }
}

impl ItemHandle {
    // задаёт длину когда приходит Content-Length; 0 → спиннер
    pub fn set_length(&self, n: u64) {
        if n > 0 {
            self.bar.set_length(n);
        } else {
            self.bar.set_style(self.spinner_style.clone());
        }
    }

    pub fn switch_to_spinner(&self) {
        self.bar.set_style(self.spinner_style.clone());
    }

    pub fn set_position(&self, n: u64) {
        self.bar.set_position(n);
    }

    pub fn inc(&self, n: u64) {
        self.bar.inc(n);
    }
}
