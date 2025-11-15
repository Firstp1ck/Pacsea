The Hungarian file is generally accurate and consistent, but there are a few clearly missing translations, some slightly misleading technical terms, and several strings left in English that should be localized.[1](config/locales/en-US.yml)[2](config/locales/hu-HU.yml)

Below is a structured review plus concrete recommended fixes you can apply to `hu-HU.yml`.[3](https://bkil.github.io/openscope-dict-eng-hun/)[2](config/locales/hu-HU.yml)[1](config/locales/en-US.yml)

### Overall assessment

- The structure of the Hungarian catalog mirrors the English one closely (same sections such as `headings`, `actions`, `details`, `modals`, `preflight`, etc.), which is good for maintainability.[2](config/locales/hu-HU.yml)[1](config/locales/en-US.yml)
- Most UI strings are understandable and idiomatic for a Hungarian user, especially core flows like search, install, remove, preflight, and system update.[2](config/locales/hu-HU.yml)
- The main issues are: some categories left untranslated, a few key technical fields where the wording is misleading, and a tail section in `key_labels`/`normal_mode` still in English with a `# TODO` comment.[3](https://bkil.github.io/openscope-dict-eng-hun/)[2](config/locales/hu-HU.yml)

### Missing or English-only strings

- Towards the end of `hu-HU.yml` there is a block with `# TODO: add translations for the missing key_labels`, where several labels are still English-only.[2](config/locales/hu-HU.yml)
- These should be localized before shipping; below are suggested Hungarian equivalents you can drop in:

- `confirm: " Confirm"` → Megerősítés (keep the leading space if it is intentional for layout).[3](https://bkil.github.io/openscope-dict-eng-hun/)[2](config/locales/hu-HU.yml)
- `remove: " Remove"` → Eltávolítás.[3](https://bkil.github.io/openscope-dict-eng-hun/)[2](config/locales/hu-HU.yml)
- `clear: " Clear"` → Törlés (matches how you already use Törlés for the `clear` action elsewhere).[3](https://bkil.github.io/openscope-dict-eng-hun/)[2](config/locales/hu-HU.yml)
- `toggle_normal: " Toggle normal"` → Normál mód váltása (literally “toggle normal mode”).[2](config/locales/hu-HU.yml)[3](https://bkil.github.io/openscope-dict-eng-hun/)
- `insert_mode: " Insert Mode"` → Beszúrás mód (consistent with existing Beszúrás mód in `actions.insert_mode`).[2](config/locales/hu-HU.yml)
- `select_left: " Select left"` → Bal kijelölése (or Bal oldal kijelölése if you prefer more explicit).[2](config/locales/hu-HU.yml)
- `select_right: " Select right"` → Jobb kijelölése.[2](config/locales/hu-HU.yml)
- `open_arch_status: " Open Arch status"` → Arch állapot megnyitása (you already use this wording elsewhere).[2](config/locales/hu-HU.yml)
- `config_lists_menu: " Config/Lists menu"` → Konfigurációk/Listák menü.[2](config/locales/hu-HU.yml)
- `options_menu: " Options menu"` → Beállítások menü.[2](config/locales/hu-HU.yml)
- `panels_menu: " Panels menu"` → Panelek menü.[2](config/locales/hu-HU.yml)

For the nested `normal_mode` block:

- `label: "Normal Mode:"` → Normál mód: (you already use Normál mód in other places, so this keeps consistency).[1](config/locales/en-US.yml)[2](config/locales/hu-HU.yml)
- `insert_mode: " Insert Mode, "` → Beszúrás mód,  (comma and spacing can stay as in English for formatting).[1](config/locales/en-US.yml)[2](config/locales/hu-HU.yml)
- `move: " move, "` → mozgatás, .[1](config/locales/en-US.yml)[2](config/locales/hu-HU.yml)
- `page: " page, "` → oldal, .[1](config/locales/en-US.yml)[2](config/locales/hu-HU.yml)
- `select_text: " Select text, "` → szöveg kijelölése, .[1](config/locales/en-US.yml)[2](config/locales/hu-HU.yml)
- `delete_text: " Delete text, "` → szöveg törlése, .[1](config/locales/en-US.yml)[2](config/locales/hu-HU.yml)
- `clear_input: " Clear input"` → bevitel törlése.[1](config/locales/en-US.yml)[2](config/locales/hu-HU.yml)

These suggestions are aligned with the rest of your Hungarian wording and standard GUI terminology used in software localization dictionaries such as OpenScope/Microsoft.[3](https://bkil.github.io/openscope-dict-eng-hun/)[2](config/locales/hu-HU.yml)

### Untranslated category labels

The `optional_deps.categories` block is explicitly marked with a TODO and is still fully in English: `editor`, `terminal`, `clipboard`, `aur_helper`, `security`.[2](config/locales/hu-HU.yml)
These appear as headings or labels for TUI optional dependencies, so they should be translated for a fully localized experience.[1](config/locales/en-US.yml)[2](config/locales/hu-HU.yml)
Good Hungarian equivalents would be:

- `editor: "Editor"` → Szerkesztő.[3](https://bkil.github.io/openscope-dict-eng-hun/)[2](config/locales/hu-HU.yml)
- `terminal: "Terminal"` → Terminál.[2](config/locales/hu-HU.yml)
- `clipboard: "Clipboard"` → Vágólap.[2](config/locales/hu-HU.yml)
- `aur_helper: "AUR Helper"` → AUR-segéd or AUR-kezelő; AUR-segéd is short and natural.[3](https://bkil.github.io/openscope-dict-eng-hun/)[2](config/locales/hu-HU.yml)
- `security: "Security"` → Biztonság.[3](https://bkil.github.io/openscope-dict-eng-hun/)[2](config/locales/hu-HU.yml)

These terms match common usage in Hungarian software UIs and in specialized terminology lists.[3](https://bkil.github.io/openscope-dict-eng-hun/)[2](config/locales/hu-HU.yml)

### Key terminology that should be tightened

Below are the most important places where the current Hungarian is understandable but semantically off or potentially confusing for a technical user.[1](config/locales/en-US.yml)[3](https://bkil.github.io/openscope-dict-eng-hun/)[2](config/locales/hu-HU.yml)

- **`details.fields.provides`**  
  - English: "Provides".[1](config/locales/en-US.yml)
  - Current: Szolgáltató, which literally reads as “provider (person/company)” rather than “things this package provides”.[2](config/locales/hu-HU.yml)
  - Suggest: Biztosítja or Nyújtja, e.g. Biztosítja (this is closer to how package managers describe virtual packages they “provide”).[3](https://bkil.github.io/openscope-dict-eng-hun/)[2](config/locales/hu-HU.yml)

- **`details.fields.replaces`**  
  - English: "Replaces".[1](config/locales/en-US.yml)
  - Current: Cserék:, which sounds like “exchanges” rather than “packages this one replaces”.[2](config/locales/hu-HU.yml)
  - Suggest: Helyettesíti or Lecseréli, e.g. Helyettesíti:, which clearly matches the semantics of replacement packages.[3](https://bkil.github.io/openscope-dict-eng-hun/)[2](config/locales/hu-HU.yml)

- **`headings.globals`**  
  - English: "GLOBALS:".[1](config/locales/en-US.yml)
  - Current: GLOBÁLIS:, which is grammatically a bare adjective and feels slightly unfinished.[2](config/locales/hu-HU.yml)
  - Suggest: Globális beállítások: (“Global settings”) or shorter Globális értékek:, depending on intended content.[3](https://bkil.github.io/openscope-dict-eng-hun/)[1](config/locales/en-US.yml)[2](config/locales/hu-HU.yml)

- **`headings.recent`, `headings.install`, etc.**  
  - English uses bare nouns like "RECENT:", "INSTALL:", "REMOVE:".[1](config/locales/en-US.yml)
  - Current: LEGUTÓBBI:, TELEPÍTÉS:, ELTÁVOLÍTÁS:, which are understandable but slightly abrupt in Hungarian.[2](config/locales/hu-HU.yml)
  - Consider plural or more explicit forms such as Legutóbbiak:, Telepítés alatt:, Eltávolítás alatt:, although this is more style than correctness.[3](https://bkil.github.io/openscope-dict-eng-hun/)[2](config/locales/hu-HU.yml)

- **`details.fields.optional_for`**  
  - English: "Optional for".[1](config/locales/en-US.yml)
  - Current: Nem kötelező ezekhez:, which is understandable but a bit clumsy.[2](config/locales/hu-HU.yml)
  - Suggest: Opcionális ezekhez: or Választható ezekhez:, which reads more naturally while preserving meaning.[3](https://bkil.github.io/openscope-dict-eng-hun/)[2](config/locales/hu-HU.yml)

- **`details.fields.required_by`**  
  - English: "Required by".[1](config/locales/en-US.yml)
  - Current: Szükséges ehhez:, which is clear but singular-ish.[2](config/locales/hu-HU.yml)
  - A slightly more literal and list-friendly variant is Szükséges a következőkhöz:, which better introduces a list of packages.[3](https://bkil.github.io/openscope-dict-eng-hun/)[2](config/locales/hu-HU.yml)

These adjustments bring the terminology closer to what Hungarian users expect from package managers and technical documentation.[3](https://bkil.github.io/openscope-dict-eng-hun/)[2](config/locales/hu-HU.yml)

### Style and minor phrasing notes

Some messages are perfectly understandable but could be polished for style and consistency with other Hungarian software.[3](https://bkil.github.io/openscope-dict-eng-hun/)[2](config/locales/hu-HU.yml)
These are optional improvements rather than “bugs”, so you can prioritize them lower.[3](https://bkil.github.io/openscope-dict-eng-hun/)[2](config/locales/hu-HU.yml)

- **`gnome_terminal_warning`**  
  - English: "Continuing without gnome-terminal may cause unexpected behavior".[1](config/locales/en-US.yml)
  - Current: Folytatás Gnome Terminál nélkül váratlan viselkedést okozhat.[2](config/locales/hu-HU.yml)
  - Suggest: A GNOME terminál nélküli folytatás váratlan viselkedést okozhat or shorter GNOME terminál nélkül a program váratlanul viselkedhet, which flows more naturally and uses consistent capitalization.[3](https://bkil.github.io/openscope-dict-eng-hun/)[2](config/locales/hu-HU.yml)

- **`gnome_terminal_prompt.body_line1`**  
  - English: "GNOME was detected, but no GNOME terminal (gnome-terminal or gnome-console/kgx) is installed.".[1](config/locales/en-US.yml)
  - Current: GNOME észlelve, de nincs telepítve GNOME Terminál (Gnome Terminál vagy Gnome Konzol/KGX)..[2](config/locales/hu-HU.yml)
  - Suggest tightening to something like A GNOME környezet felismerve, de nincs telepítve GNOME terminál (gnome-terminal vagy gnome-console/kgx)., which avoids repetition and matches typical terminology.[3](https://bkil.github.io/openscope-dict-eng-hun/)[2](config/locales/hu-HU.yml)

- **`preflight.summary.no_conflicts_or_upgrades`**  
  - English: "No conflicts or upgrades required.".[1](config/locales/en-US.yml)
  - Current: Nincs szükség ütközéskezelésre vagy frissítésre..[2](config/locales/hu-HU.yml)
  - This is fine, but a slightly simpler alternative is Nincsenek ütközések, és nincs szükség frissítésre., which is closer to the source structure.[3](https://bkil.github.io/openscope-dict-eng-hun/)[2](config/locales/hu-HU.yml)

- **Use of “Szimulált futtatás” for “Dry-run mode”**  
  - English: "Dry-run mode enabled/disabled".[1](config/locales/en-US.yml)
  - Current: Szimulált futtatás engedélyezve / Szimulált futtatás letiltva.[2](config/locales/hu-HU.yml)
  - This is understandable; if you want a more idiomatic term, Próbafuttatási mód engedélyezve/letiltva is also common in Hungarian technical texts.[3](https://bkil.github.io/openscope-dict-eng-hun/)[2](config/locales/hu-HU.yml)

### References

[1](config/locales/en-US.yml)
[2](config/locales/hu-HU.yml)
[3](https://bkil.github.io/openscope-dict-eng-hun/)
[4](https://bkil.gitlab.io/openscope-dict-eng-hun/)
[5](https://learn.microsoft.com/en-us/globalization/reference/microsoft-terminology)
[6](https://learn.microsoft.com/en-us/azure/ai-services/translator/text-translation/reference/v3/dictionary-lookup)
[7](https://learn.microsoft.com/en-us/security-updates/glossary/glossary)
[8](https://www.slideshare.net/slideshow/microsoft-abbreviations-dictionary-72873305/72873305)
[9](https://gitlab.com/bkil/openscope-dict-eng-hun)
[10](https://learn.microsoft.com/en-us/windows/win32/api/msime/nf-msime-ifedictionary-open)
[11](https://bkil.gitlab.io/secuchart/)
[12](https://parker-translation.com/site/documents/Microsoft%20Terminology.csv)
[13](https://wiki.mageia.org/en/Hungarian_i18n_subteam)
[14](https://www.scribd.com/document/656377060/Msft-Eng-spa-Glossary-1686312028)
[15](https://gitlab.liu.se/gusli687/openscope-attacks/-/blob/develop/README.md)
[16](https://www.dotnetframework.de/glossar/default.aspx)
[17](https://gitlab.liu.se/openscope/openscope-attack-simulator/-/tree/default/src/templates)
[18](https://learn.microsoft.com/en-us/dotnet/api/system.windows.markup.xmlnsdictionary.popscope?view=windowsdesktop-10.0)
[19](https://gitlab.liu.se/gusli687/openscope-attacks/-/tree/v6.9.1/documentation)