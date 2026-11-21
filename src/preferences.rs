use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    System,
    English,
    Russian,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locale {
    English,
    Russian,
}
impl Locale {
    fn detect<'a>(languages: impl IntoIterator<Item = &'a str>) -> Self {
        for language in languages {
            let tag = language.split(['_', '-', '.', '@']).next().unwrap_or("");
            if tag.eq_ignore_ascii_case("ru") {
                return Self::Russian;
            }
            if tag.eq_ignore_ascii_case("en") || tag.eq_ignore_ascii_case("C") || tag == "POSIX" {
                return Self::English;
            }
        }
        Self::English
    }
    pub fn pair(self, en: &'static str, ru: &'static str) -> &'static str {
        match self {
            Self::English => en,
            Self::Russian => ru,
        }
    }
}
impl Language {
    pub fn locale(self) -> Locale {
        match self {
            Self::English => Locale::English,
            Self::Russian => Locale::Russian,
            Self::System => {
                static LOCALE: std::sync::OnceLock<Locale> = std::sync::OnceLock::new();
                *LOCALE.get_or_init(|| Locale::detect(gtk::glib::language_names().iter().map(|s| s.as_str())))
            }
        }
    }
}

macro_rules! messages {
    ($($id:ident => $en:literal, $ru:literal;)+) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Text { $($id),+ }
        impl Locale {
            pub fn get(self, id: Text) -> &'static str {
                match id { $(Text::$id => self.pair($en, $ru)),+ }
            }
        }
    }
}
messages! {
    Library => "Library", "Библиотека";
    Preferences => "Settings", "Настройки";
    Diagnostics => "Diagnostics", "Диагностика";
    Import => "Import file…", "Добавить файл…";
    Formats => "Images, videos and GIF", "Изображения, видео и GIF";
    Open => "Open", "Открыть";
    Save => "Save", "Сохранить";
    Export => "Export report", "Сохранить отчёт";
    Empty => "Import an image, video or GIF to choose your wallpaper.", "Добавьте изображение, видео или GIF, чтобы выбрать обои.";
    Preview => "Play preview", "Воспроизвести";
    Stop => "Stop preview", "Остановить";
    Screen => "Display", "Монитор";
    All => "All monitors", "Все мониторы";
    Disconnected => "Selected monitor is disconnected", "Выбранный монитор отключён";
    Scaling => "Scaling", "Масштабирование";
    Fill => "Fill", "Заполнить";
    Fit => "Fit", "Вписать";
    Apply => "Apply", "Применить";
    ClearWallpapers => "Remove wallpaper from all monitors", "Убрать фон со всех мониторов";
    DeleteItem => "Delete from library…", "Удалить из библиотеки…";
    Delete => "Delete", "Удалить";
    Cancel => "Cancel", "Отмена";
    ConfirmDelete => "Delete the selected copy from the library?", "Удалить выбранную копию из библиотеки?";
    ConfirmDeleteActive => "Delete this wallpaper and remove it from the displays using it?", "Удалить эти обои из библиотеки и снять их с мониторов, где они используются?";
    Deleted => "Deleted from the library", "Обои удалены из библиотеки";
    DeleteCleanup => "Removed from the library. File cleanup will retry at startup.", "Обои убраны из библиотеки. Удаление файлов повторится при запуске.";
    FailedDelete => "Cannot delete this wallpaper. See Diagnostics.", "Не удалось удалить обои. Подробности в диагностике.";
    Appearance => "Theme", "Тема";
    Language => "Language", "Язык";
    SystemTheme => "System", "Системная";
    SystemLanguage => "System", "Системный";
    Light => "Light", "Светлая";
    Dark => "Dark", "Тёмная";
    Login => "Restore animation at login", "Восстанавливать анимацию при входе";
    Power => "Pause in power saver mode", "Приостанавливать в режиме энергосбережения";
    Pause => "Pause wallpaper animation", "Приостановить анимацию обоев";
    PauseInOverview => "Pause animation in the GNOME overview", "Приостанавливать анимацию в обзоре GNOME";
    Imported => "Saved to your library", "Файл добавлен в библиотеку";
    Applied => "Wallpaper applied", "Обои применены";
    Cleared => "Wallpaper removed", "Фон убран";
    Exported => "Report saved", "Отчёт сохранён";
    Damaged => "Cannot load the saved profile. See Diagnostics.", "Не удалось прочитать профиль. Подробности в диагностике.";
    FailedImport => "Cannot import the file. See Diagnostics.", "Не удалось добавить файл. Подробности в диагностике.";
    FailedApply => "Cannot apply wallpaper. See Diagnostics.", "Не удалось применить обои. Подробности в диагностике.";
    FailedClear => "Cannot remove wallpaper. See Diagnostics.", "Не удалось убрать фон. Подробности в диагностике.";
    FailedSave => "Cannot save settings. See Diagnostics.", "Не удалось сохранить настройки. Подробности в диагностике.";
    FailedExport => "Cannot save the report. See Diagnostics.", "Не удалось сохранить отчёт. Подробности в диагностике.";
    FailedPreview => "Cannot play the preview. See Diagnostics.", "Не удалось воспроизвести предпросмотр. Подробности в диагностике.";
    FailedService => "Wallpaper service reported an error. See Diagnostics.", "Ошибка фоновой службы. Подробности в диагностике.";
    DiskFull => "The library disk is full.", "На диске библиотеки закончилось место.";
    Unsupported => "Unsupported media format.", "Этот формат файла не поддерживается.";
    EmptyFile => "Select a non-empty file.", "Выберите непустой файл.";
    Missing => "Library file is missing.", "Файл отсутствует в библиотеке.";
    Extension => "Enable the Ubrus GNOME Shell extension. See Diagnostics.", "Включите расширение Ubrus для GNOME Shell. Инструкция есть в диагностике.";
    Declined => "The desktop declined the wallpaper change.", "Рабочий стол отклонил смену обоев.";
    SaveAs => "Save as…", "Сохранить как…";
    CopyAll => "Copy all", "Скопировать всё";
    Refresh => "Refresh", "Обновить";
    Guide => "Enable Ubrus in GNOME Extensions. With the standalone binary, install ubrus-extension.zip from its directory using gnome-extensions install ./ubrus-extension.zip. With Snap, run gnome-extensions install /snap/ubrus/current/share/ubrus/ubrus-extension.zip. Log out and back in, then enable the extension.", "Включите Ubrus в приложении «Расширения». Для отдельного бинарника выполните в его каталоге gnome-extensions install ./ubrus-extension.zip. Для Snap выполните gnome-extensions install /snap/ubrus/current/share/ubrus/ubrus-extension.zip. После установки выйдите из сеанса, войдите снова и включите расширение.";
}

#[cfg(test)]
#[path = "../tests/rust/preferences.rs"]
mod tests;
