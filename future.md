components:
- tree (z rozwijanymi elementami + może dodwaaniem nowych?)
- file browser z filtrem i autocomplete i ewentualnym tworzeniem plików/folderów
- table 
- snippet
- key/value edytor
- json edytor z definiowaniem pola
- records
- searchlist
- -async select
- kalendarz
- textarea

outputs:
- text diff
- table
- output do pliku z naniesionymi zmianami
- spinner z outputem logów (może też być kilka STEPS[1/n], coś jak docker)
- kopiowanie do schowka
- paste aware różnych formatów
- repeater


common:
- undo/redo
- autofill
- pager (scrollable)
- filter
- multiline input
- -HELP?


DateInput` / `TimeInput` — oparte na masked, ale z walidacją zakresu i nawigacją strzałkami po polach (dd/mm/yyyy)
- `NumberInput` — dedykowany input numeryczny z min/max, step, formatowaniem
- `TagInput` — jak ArrayInput ale tagi wyświetlane jako `[tag1] [tag2]` inline, usuwanie Backspace
