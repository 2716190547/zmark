# Willkommen bei ZMark

![ZMark Geteilter Echtzeit-Arbeitsbereich](zmark-asset://welcome/hero.svg)

> Ein schneller, nativer lokaler Markdown-Editor und Lese-Arbeitsbereich auf Basis von **Rust + GPUI**.<br/>Bearbeiten Sie beliebigen Markdown-Text links und erleben Sie die blockgenau synchronisierte Vorschau rechts.

[toc]

---

## 1. Was ZMark alles kann

1. **Dateien & Entwürfe verwalten**: Öffnen Sie mit `Cmd+O` lokale `.md`-Dateien, erstellen Sie mit `Cmd+N` Entwürfe oder **ziehen Sie Dateien, Ordner und Bilder per Drag & Drop** direkt ins Fenster.
2. **Drei Ansichtsmodi**: Wechseln Sie blitzschnell zwischen **Quelltext** (`Cmd+1`), **Geteilt** (`Cmd+2`) und **Vorschau** (`Cmd+3`) mit weichem Synchronscrollen.
3. **Seitenleiste & Klickbares Inhaltsverzeichnis**: `Cmd+Shift+B` öffnet die Seitenleiste, `Cmd+Shift+O` wechselt zwischen **Dateien** und **Gliederung** — und `[toc]` rendert direkt eine kompakte Sprungliste im Text.
4. **Speichern, Umbenennen & HTML-Export**: `Cmd+S` speichert, ein **Doppelklick auf den Titel** benennt um und `Cmd+Shift+E` exportiert eigenständiges HTML.
5. **7 Farbpaletten & freie Tastenkürzel**: `Cmd+,` öffnet die Einstellungen mit 19 Farbwerten und **17 per Klick neu aufnehmbaren Tastenkürzeln**.
6. **Vektor-SVG & HiDPI-Leuchtkasten**: Klicken Sie auf lokale Bilder, Vektorgrafiken oder Mermaid-Diagramme, um sie im Leuchtkasten stufenlos zu zoomen.

---

## 2. Neu in ZMark

Probieren Sie jeden Punkt aus und ändern Sie danach `[ ]` links im Quelltext zu `[x]`:

- [ ] **Getrennte Schriften für Text und Code**: Öffnen Sie mit `Cmd+,` die Einstellungen und wählen Sie unter **Erscheinungsbild → Schriften** eine **Textschrift** und eine **Codeschrift**. Die Listen durchsuchen die installierten Schriften. Die Textschrift gilt auch für die Oberfläche.
- [ ] **Lesebreite ziehen**: Wechseln Sie mit `Cmd+3` in die Vorschau und ziehen Sie einen Rand der Textspalte. Ein Doppelklick auf den Rand stellt die Standardbreite wieder her. Unter **Editor & Speichern → Spaltenbreite** gibt es auch Voreinstellungen.
- [ ] **Seitenleiste verbreitern**: Blenden Sie sie mit `Cmd+Shift+B` ein und ziehen Sie den rechten Rand. Ein Doppelklick stellt die Standardbreite wieder her.
- [ ] **Häufige Dateien anheften**: Klicken Sie in der Seitenleiste mit der rechten Maustaste auf eine Datei oder einen Ordner und wählen Sie **Oben anheften**. Der Eintrag steht dann oben in seinem Ordner.
- [ ] **Gesten im Leuchtkasten**: Klicken Sie weiter unten auf eine Illustration oder ein Diagramm. Zoomen Sie mit dem Mausrad oder durch Zusammenziehen auf dem Trackpad und verschieben Sie mit zwei Fingern auf einem Mac-Trackpad. Die Scrollrichtung folgt der Systemeinstellung.
- [ ] **Weiterarbeiten, wo Sie aufgehört haben**: Beim Start öffnet ZMark die Dateien, die beim letzten Beenden offen waren. Ein Doppelklick auf eine `.md`-Datei im Finder oder Dateimanager öffnet genau diese Datei.

> **Zu diesem Leitfaden:** Nachdem Sie die Willkommensseite geschlossen haben, erscheint sie nicht mehr von selbst. Um sie wieder zu öffnen, gehen Sie zu **Einstellungen → Über → Willkommensleitfaden** und klicken auf **Leitfaden wiederherstellen**.

---

## 3. Interaktiver Markdown-Syntax-Baukasten

### 1. Textformatierung, Zeilenumbrüche, Zitate & Links

- Auszeichnungen: **Fett**, *Kursiv*, ~~Durchgestrichen~~ und `Inline-Code`.
- HTML `<br/>`: `<br/>` im Fließtext erzwingt einen Umbruch, allein in einer Zeile schafft es großzügigen Abstand.
- Kürzel & Suche: `Cmd+B` für Fett, `Cmd+Shift+7` für nummerierte Listen, `Cmd+F` (`Cmd+G` / `Cmd+Shift+G`) für Suche.
- Links: [ZMark auf GitHub](https://github.com/2716190547/zmark) · [Releases](https://github.com/2716190547/zmark/releases)

> **100 % lokal:** ZMark rendert adaptive Tabellen, Aufgabenlisten, mehrsprachiges Syntax-Highlighting, SVGs und Mermaid-Diagramme offline in Millisekunden.

### 2. Listen & interaktive Aufgaben

- [x] ZMark starten und `Willkommen bei ZMark.md` ansehen
- [ ] Oben im `[toc]`-Kasten auf eine Überschrift klicken, um dorthin zu springen
- [ ] Seitenleiste mit `Cmd+Shift+B` öffnen und einen Ordner hinzufügen
- [ ] In `Cmd+,` ein Design wählen oder ein Tastenkürzel neu belegen
  - Verschachtelte Listenpunkte rücken automatisch ein
  - Ersetzen Sie `[ ]` links durch `[x]`, um Aufgaben live abzuhaken

### 3. Adaptive Tabellen & Standardkürzel

| Funktion | Standardkürzel (Mac / Win & Linux) | Beschreibung |
| --- | --- | --- |
| Öffnen / Neuer Entwurf | `Cmd+O` / `Cmd+N` | Dateien und ungespeicherte Entwürfe parallel nutzen |
| Quelltext / Geteilt / Vorschau | `Cmd+1` / `Cmd+2` / `Cmd+3` | Weiches, blockgenaues Synchronscrollen |
| Seitenleiste / Gliederung | `Cmd+Shift+B` / `Cmd+Shift+O` | Ordnerbaum, Suche & Überschriften-Sprung |
| Im Dokument suchen | `Cmd+F` (`Cmd+G` weiter) | Live-Trefferzahl und schnelle Navigation |
| Eigenständiges HTML exportieren | `Cmd+Shift+E` | Eigenständiges HTML mit integriertem Stylesheet |
| Einstellungen öffnen | `Cmd+,` | Paletten, Typografie & Tastenkürzel-Editor |

### 4. Codeblöcke mit Syntax-Highlighting

```rust
fn main() {
    let app = "ZMark";
    let stack = ["Rust", "GPUI", "pulldown-cmark", "rusty-mermaid"];
    println!("{app} built with {stack:?}");
}
```

```json
{
  "editor": "ZMark",
  "views": ["source", "split", "preview"],
  "tocBlock": true,
  "vectorSvgLightbox": true
}
```

### 5. Ausprobieren: Vektor-Postkarte unten zum Zoomen anklicken!

![Vektor-Postkarte · Zum Vergrößern im Leuchtkasten klicken](zmark-asset://welcome/lightbox-badge.svg)

<br/>

### 6. Native Mermaid-Diagramme (ebenfalls klickbar)

```mermaid
flowchart LR
    Write["Markdown & Mermaid schreiben"] --> Parse["Schneller lokaler AST"]
    Parse --> Split["Weiche Geteilt-Vorschau"]
    Parse --> Outline["Gliederung & TOC-Sprung"]
    Split --> Export["Eigenständiger HTML-Export"]
    Split --> Lightbox["Zum Zoomen anklicken"]
```
