progress nowy
collapsible
timer
split view
write to file

- naprawa SCOLL na kursor + dodanie że komponenty np. select list mają kursto tam gdzie potrzeba
- 
- select list może dostać obj ale też str[]
- commands runners bez outputu/only command  
- confirm z customowymi yes/no + poprawa podświetlenia + default opcja
- table movement do refactoru
- zastanowić się kiedy enter może być toggle a kiedy zmienia focus/step
- output do file
- repeater?
- takski działające per oninput powinny być triggerowane na select list toggle
- zmiana z oninptu na onchange event
- jeżeli brak bindingu to czerwony error w tym miejscu





-a może byśmy dodali jakieś ostrzeżenie że usuwamy binding w runtime? np. jak naciskam backspace i dojadę do wartości "zaciąganej" przez {{}} to ona się pogrubia i muszę raz jeszcze dodatkowo wcisnąć backspace żeby usunąć - wtedy świadomie usuwam ten binding. A jak zmienie  albo dopisze coś przed/po bindingu to poxniejsza zmiana w reads tylko będzie aktualizowana. Co myślisz o tym? to już przekombinowane?
