#!/usr/bin/env python3
"""Remove only GND vias reported by KiCad as connected on one layer after refill.

Usage: KiCadPython remove_unused_vias.py path/to/current-drc.json
Requires fresh DRC against this exact board. Saves an audit of removed positions.
"""
from pathlib import Path
import sys,json
import pcbnew as k
ROOT=Path(__file__).resolve().parent
report=json.loads(Path(sys.argv[1]).read_text())
b=k.LoadBoard(str(ROOT/'imu_to_dxl.kicad_pcb'))
ids={i['uuid'] for v in report['violations'] if v['type']=='via_dangling' for i in v['items']}
removed=[]
for t in list(b.GetTracks()):
    if t.m_Uuid.AsString() not in ids:continue
    assert isinstance(t,k.PCB_VIA) and t.GetNetname()=='GND'
    removed.append({'net':'GND','x_mm':k.ToMM(t.GetPosition().x)-100,'y_mm_from_bottom':122-k.ToMM(t.GetPosition().y),'diameter_mm':k.ToMM(t.GetWidth(k.F_Cu)),'drill_mm':k.ToMM(t.GetDrill())})
    b.Remove(t)
assert len(removed)==len(ids), 'DRC report does not match the current board'
k.SaveBoard(str(ROOT/'imu_to_dxl.kicad_pcb'),b)
(ROOT/'checks/removed_unused_vias.json').write_text(json.dumps({'reason':'KiCad via_dangling after ground pour refill; GND vias had no connection on F.Cu. No signal or power trace removed.','count':len(removed),'vias':removed},indent=2))
print('Removed',len(removed),'redundant GND vias')
