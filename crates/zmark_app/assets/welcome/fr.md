# Bienvenue dans ZMark

![Espace de travail partagé ZMark](zmark-asset://welcome/hero.svg)

> Un éditeur Markdown local et espace de lecture natif ultra-rapide conçu avec **Rust + GPUI**.<br/>Modifiez librement ce document à gauche et observez l'aperçu synchronisé répondre bloc par bloc à droite.

[toc]

---

## 1. Prise en main : capacités clés de ZMark

1. **Fichiers locaux et brouillons** : `Cmd+O` ouvre un `.md`, `Cmd+N` crée un brouillon, ou **glissez-déposez des fichiers Markdown, dossiers et images** dans la fenêtre.
2. **Trois modes de vue** : alternez entre **Source** (`Cmd+1`), **Vue partagée** (`Cmd+2`) et **Aperçu** (`Cmd+3`) avec défilement fluide et synchronisé.
3. **Barre latérale et sommaire interactif** : `Cmd+Shift+B` ouvre la barre latérale et `Cmd+Shift+O` bascule entre **Fichiers** et **Plan** ; un marqueur `[toc]` génère aussi un bloc sommaire cliquable dans le document.
4. **Enregistrement, renommage et export HTML** : `Cmd+S` enregistre, un **double-clic sur le titre** renomme sur place, et `Cmd+Shift+E` exporte un HTML autonome.
5. **7 palettes et raccourcis personnalisables** : `Cmd+,` ouvre les paramètres avec 19 couleurs éditables et **17 raccourcis enregistrables au clic**.
6. **SVG vectoriel et visionneuse HiDPI** : cliquez sur une image locale, une illustration SVG ou un diagramme Mermaid pour l'ouvrir dans la visionneuse zoomable.

---

## 2. Nouveautés

Essayez chaque point, puis remplacez son `[ ]` par `[x]` dans la source à gauche :

- [ ] **Polices du texte et du code séparées** : ouvrez les réglages avec `Cmd+,`, puis choisissez la **Police du texte** et la **Police du code** dans **Apparence → Polices**. Les listes permettent de rechercher les polices installées. La police du texte sert aussi à l'interface.
- [ ] **Largeur de lecture à la souris** : passez en aperçu avec `Cmd+3`, puis faites glisser un bord de la colonne de texte. Un double-clic sur le bord rétablit la largeur par défaut. **Éditeur & Sauvegarde → Largeur de colonne** propose aussi des préréglages.
- [ ] **Largeur de la barre latérale** : affichez-la avec `Cmd+Shift+B`, puis faites glisser son bord droit. Un double-clic rétablit la largeur par défaut.
- [ ] **Épingler les fichiers fréquents** : faites un clic droit sur un fichier ou un dossier de la barre latérale et choisissez **Épingler en haut**. Il passe en tête de son dossier.
- [ ] **Gestes dans la visionneuse** : cliquez sur une illustration ou un diagramme plus bas. Zoomez avec la molette ou en pinçant le trackpad, et déplacez l'image à deux doigts sur un trackpad de Mac. Le sens du défilement suit le réglage du système.
- [ ] **Activer la barre d'onglets** : appuyez sur `Cmd+,` pour ouvrir les réglages et activez **Afficher la barre d'onglets** dans **Éditeur et enregistrement**. Les documents ouverts deviennent des onglets dans l'en-tête ; glissez pour les réordonner, `Ctrl+Tab` pour passer de l'un à l'autre.
- [ ] **Modifier directement dans l'aperçu** : sur la même page, activez **Modifier dans l'aperçu**, appuyez sur `Cmd+3` pour l'aperçu, puis cliquez sur un paragraphe, une liste ou un bloc de code. Il se transforme sur place en son Markdown source ; `Échap` pour terminer.
- [ ] **Ouvrir des fichiers rapidement** : `Cmd+O` pour choisir un fichier, ou **Ouvrir un fichier ou dossier** en haut de la barre latérale pour choisir un fichier ou ajouter un dossier entier. Vous pouvez aussi **glisser** un fichier `.md` ou un dossier depuis le Finder ou votre gestionnaire de fichiers dans la fenêtre ; un dossier apparaît dans la barre latérale.
- [ ] **Reprendre là où vous en étiez** : au lancement, ZMark rouvre les fichiers ouverts lors de la dernière fermeture. Un double-clic sur un fichier `.md` dans le Finder ou le gestionnaire de fichiers ouvre directement ce fichier.

> **À propos de ce guide :** une fois la page d'accueil fermée, elle ne s'ouvre plus d'elle-même. Pour la revoir, allez dans **Réglages → À propos → Guide de bienvenue** et cliquez sur **Restaurer le guide**.

---

## 3. Galerie interactive de syntaxe Markdown

### 1. Mise en forme, retours à la ligne, citations et liens

- Styles en ligne : **gras**, *italique*, ~~barré~~ et `code en ligne`.
- Balises HTML `<br/>` : utilisez `<br/>` dans un paragraphe pour forcer un retour ou seul sur une ligne pour aérer.
- Édition et recherche : `Cmd+B` pour le gras, `Cmd+Shift+7` pour liste numérotée, `Cmd+F` (`Cmd+G` / `Cmd+Shift+G`) pour chercher.
- Liens : [Dépôt GitHub ZMark](https://github.com/2716190547/zmark) · [Versions publiées](https://github.com/2716190547/zmark/releases)

> **100 % local :** ZMark rend tableaux adaptatifs, tâches, code coloré, SVG vectoriels et diagrammes Mermaid hors ligne en quelques millisecondes.

### 2. Listes et tâches interactives

- [x] Lancer ZMark et découvrir `Bienvenue dans ZMark.md`
- [ ] Cliquer sur un titre du sommaire `[toc]` ci-dessus pour y sauter
- [ ] Appuyer sur `Cmd+Shift+B` pour ouvrir la barre latérale et ajouter un dossier
- [ ] Ouvrir `Cmd+,` pour changer de palette ou enregistrer vos propres raccourcis
  - Sous-éléments imbriqués avec indentation automatique
  - Remplacez `[ ]` par `[x]` à gauche pour cocher la tâche en direct

### 3. Tableaux adaptatifs et raccourcis par défaut

| Fonction | Raccourci (Mac / Win & Linux) | Détails |
| --- | --- | --- |
| Ouvrir / Nouveau brouillon | `Cmd+O` / `Cmd+N` | Travaillez sur vos dossiers et brouillons |
| Source / Partagé / Aperçu | `Cmd+1` / `Cmd+2` / `Cmd+3` | Défilement synchronisé et fluide |
| Barre latérale / Plan | `Cmd+Shift+B` / `Cmd+Shift+O` | Arborescence, filtre et titres cliquables |
| Rechercher dans le fichier | `Cmd+F` (`Cmd+G` suivant) | Compteur en direct et saut rapide |
| Export HTML autonome | `Cmd+Shift+E` | Page HTML autonome avec styles intégrés |
| Paramètres | `Cmd+,` | Couleurs personnalisées et raccourcis |

### 4. Blocs de code avec coloration syntaxique

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

### 5. À vous : cliquez sur l'illustration SVG pour zoomer !

![Carte postale vectorielle · Cliquer pour ouvrir la visionneuse](zmark-asset://welcome/lightbox-badge.svg)

<br/>

### 6. Diagrammes Mermaid natifs (cliquer aussi pour zoomer)

```mermaid
flowchart LR
    Write["Écrire Markdown & Mermaid"] --> Parse["Analyse locale rapide"]
    Parse --> Split["Aperçu partagé fluide"]
    Parse --> Outline["Plan & sommaire TOC"]
    Split --> Export["Export HTML autonome"]
    Split --> Lightbox["Zoom en visionneuse"]
```
