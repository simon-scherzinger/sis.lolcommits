# lolcommits im Team: Datenschutz

Diese Seite ist für alle, die die geteilte Galerie in einer Firma einführen wollen.
Sie ersetzt keine Rechtsberatung. Stimmt den Einsatz mit eurer oder eurem Datenschutzbeauftragten ab.

## Worum es geht

Lokal bleiben die Bilder auf dem eigenen Rechner. Das ist privat, und niemand sonst sieht sie.

Sobald Bilder geteilt werden, verarbeitet die Firma personenbezogene Daten von Beschäftigten: Gesicht, Name, Datum und Commit-Nachricht.
Wer die Bilder einzelner Personen nebeneinander legt, kann daraus Arbeitszeiten ablesen. Genau das soll nicht passieren.

## Wie lolcommits das löst

| Anforderung | Umsetzung |
|---|---|
| Einwilligung, freiwillig (Art. 6 Abs. 1 lit. a DSGVO, § 26 BDSG) | Teilen ist aus, bis jede Person es selbst einschaltet. Ein Spitzname reicht als Name. Wer nicht mitmacht, hat keinen Nachteil. |
| Jederzeit widerrufbar, echtes Löschen (Art. 7 Abs. 3, Art. 17) | In der App: Privacy › Unshare all löscht sofort alle eigenen Bilder aus allen Repo-Ordnern (in der Kommandozeile `lolcommits unshare --all`). „Delete all" löscht zusätzlich alles lokal. Bei Google Drive for desktop – und bei „Mit Google anmelden“, wenn die eigene Rolle in der geteilten Ablage kein endgültiges Löschen erlaubt (unter „Verwalter“) – landet Gelöschtes 30 Tage im Papierkorb der geteilten Ablage. |
| Datenminimierung (Art. 5 Abs. 1 lit. c) | Hochgeladen werden nur das Bild ohne Metadaten, das Datum ohne Uhrzeit, der Name und die Commit-Nachricht. Repo und Commit-Hash sind im Bild verwischt. Die Repo-Ordner tragen nur dann den Repo-Namen, wenn die Person das für dieses Repo erlaubt hat; sonst landet das Bild in `other-repos`. Wichtig für Kundenprojekte. |
| Speicherbegrenzung (Art. 5 Abs. 1 lit. e) | Geteilte Bilder laufen standardmäßig **nicht** ab, damit nichts ungewollt verschwindet. Die Frist legt das Team fest (siehe Checkliste) und stellt sie in der App einmal für alle ein: Settings › „Team: shared pictures stay at most" (gilt als Obergrenze für alle, gespeichert im geteilten Ordner unter `.meta/retention`). Jede:r kann zusätzlich eine kürzere eigene Frist setzen: „Remove my shared pictures after" (`lolcommits.share.retention`). |
| Unbeteiligte im Bild | Jedes Bild wird vor dem Upload so angezeigt, wie es hochgeladen wird: im Review-Stapel als Karte (Wisch nach rechts teilt, nach links bleibt es privat) oder im Teilen-Dialog mit zwei Häkchen. Vor dem ersten Wisch erklärt die App, worauf man achten soll: Hintergrund und Commit-Nachricht. Der letzte Wisch lässt sich rückgängig machen. |
| Keine Verhaltens- oder Leistungskontrolle (§ 87 Abs. 1 Nr. 6 BetrVG) | Es gibt keine Statistiken pro Person und keine Uhrzeiten. Der Zweck ist ausschließlich Spaß und Teamkultur. |
| Keine Biometrie, keine Emotionserkennung (Art. 9 DSGVO, Art. 5 Abs. 1 lit. f AI Act) | Es gibt keine Gesichtserkennung und keine Stimmungsanalyse. Solche Funktionen dürfen auch nicht nachgerüstet werden. |
| Zugriff nur fürs Team | Die Bilder liegen in einer geteilten Ablage. Zugriff hat nur die Team-Gruppe, nichts ist öffentlich. |

## Checkliste vor dem Rollout

- [ ] **Aufbewahrungsfrist festlegen**, etwa 12 Monate, und im Team kommunizieren. Die App hat keine Frist voreingestellt. Eine Person stellt sie in den Settings unter „Team: shared pictures stay at most“ für alle ein, oder eine verantwortliche Person räumt den Ordner regelmäßig auf.
- [ ] Datenschutzbeauftragte:n informieren und den Zweck festhalten: Teamkultur, keine Auswertung.
- [ ] Eintrag ins Verzeichnis der Verarbeitungstätigkeiten (Art. 30) anlegen.
- [ ] Betriebsrat einbinden, falls es einen gibt. Die Bilder wären technisch für eine Verhaltenskontrolle geeignet, darum ist er mitbestimmungspflichtig.
- [ ] Speicherort festlegen:
  - Google Drive: geteilte Ablage, Mitglieder = Team-Gruppe, AVV (Google Workspace DPA) liegt vor, Datenregion prüfen.
  - S3 oder eigener Server: EU-Region, AVV mit dem Hoster.
- [ ] Rechte setzen: Mitglieder brauchen "Mitwirkende:r" (Contributor), um Dateien abzulegen. Wer Verwalter:in oder Inhaltsmanager:in ist, kann technisch auch fremde Ordner löschen. Haltet diese Gruppe klein.
- [ ] Den Infotext unten anpassen und verteilen.
- [ ] Festlegen, wer die Ordner von Ausgeschiedenen löscht.
  - Eine eingestellte Frist greift nur, wenn die Person selbst noch etwas teilt.
  - Darum: einmal im Quartal alte Ordner prüfen.

## Vorlage: Infotext für das Team

> **lolcommits – freiwillig, für den Spaß**
>
> Wer mag, kann sein Commit-Selfie in unsere Team-Galerie stellen. Das ist freiwillig. Wer nicht mitmacht, hat keinen Nachteil.
>
> **Was geteilt wird:** nur Bilder, die du nach einer Vorschau selbst freigibst. Dazu kommen das Datum ohne Uhrzeit, dein angezeigter Name (Spitzname geht) und die Commit-Nachricht. Metadaten sind entfernt. Ob der Repo-Name sichtbar ist, entscheidest du pro Repo. Sonst ist er verwischt und das Bild landet unter „other-repos“.
>
> **Wo:** in der geteilten Ablage „lolcommits" in Google Drive. Zugriff hat nur [Team/Gruppe].
>
> **Wie lange:** [Frist, z. B. 12 Monate]. Danach räumen wir den Ordner auf. Du kannst in der App auch selbst eine kürzere Frist einstellen.
>
> **Wofür nicht:** keine Auswertung von Arbeitszeiten oder Leistung, keine Gesichtserkennung, keine Weitergabe.
>
> **Widerruf:** jederzeit in der App unter Privacy › Unshare all. Deine Bilder werden sofort gelöscht. Oder sag [Ansprechpartner:in] Bescheid.
>
> **Fragen:** [Datenschutzbeauftragte:r, Kontakt]
