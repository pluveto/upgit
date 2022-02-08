set "UPGIT_BIN=C:\repo\upgit\upgit.exe"
set "SNIPASTE_BIN=C:\app\Snipaste\Snipaste.exe"
%SNIPASTE_BIN% snip
%UPGIT_BIN% :clipboard --output-type clipboard --output-format markdown
pause