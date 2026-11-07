#!/usr/bin/env python3
"""Place legible reference designators clear of pads, holes, and source silkscreen.

Run after build_kicad.py using KiCad's bundled Python. Geometry is checked again
by KiCad DRC. This only edits silkscreen and cosmetic footprint reference fields.
"""
from pathlib import Path
import json, math
import pcbnew as k

ROOT=Path(__file__).resolve().parent
b=k.LoadBoard(str(ROOT/'imu_to_dxl.kicad_pcb'))
obstacles=[]
def bbox(obj,gap=0):
    z=obj.GetBoundingBox()
    return (k.ToMM(z.GetLeft())-gap,k.ToMM(z.GetTop())-gap,k.ToMM(z.GetRight())+gap,k.ToMM(z.GetBottom())+gap)
def overlap(a,b):return a[0]<b[2] and b[0]<a[2] and a[1]<b[3] and b[1]<a[3]
def refbox(x,y,text):
    # Bounding boxes for text are obtained from KiCad below, not estimated by string length.
    text.SetPosition(k.VECTOR2I(k.FromMM(x),k.FromMM(y)))
    return bbox(text,.1)
for fp in b.GetFootprints():
    for p in fp.Pads():obstacles.append(bbox(p,.19))
    fp.Reference().SetVisible(False)
    for g in list(fp.GraphicalItems()):
        if g.GetLayer()==k.F_SilkS:
            # Keep source assembly outlines in F.Fab. Native silk contains refs and pin-one markers.
            g.SetLayer(k.F_Fab)
fps=[fp for fp in b.GetFootprints() if not fp.GetReference().startswith('MH')]
fps.sort(key=lambda f:(0 if f.GetReference().startswith(('U','J','X')) else 1,f.GetReference()))
positions={}
for fp in fps:
    text=fp.Reference();text.SetVisible(True);text.SetLayer(k.F_SilkS)
    text.SetTextSize(k.VECTOR2I(k.FromMM(.8),k.FromMM(.8)));text.SetTextThickness(k.FromMM(.12));text.SetTextAngle(k.EDA_ANGLE(0,k.DEGREES_T))
    cx,cy=k.ToMM(fp.GetPosition().x),k.ToMM(fp.GetPosition().y)
    candidates=[(cx+dx*.25,cy+dy*.25) for dx in range(-24,25) for dy in range(-24,25)]
    candidates.sort(key=lambda p:(p[0]-cx)**2+(p[1]-cy)**2+.1*max(0,p[1]-cy))
    for x,y in candidates:
        bb=refbox(x,y,text)
        if bb[0]<100.5 or bb[1]<100.5 or bb[2]>144.5 or bb[3]>121.5:continue
        if any(overlap(bb,o) for o in obstacles):continue
        obstacles.append(bb);positions[fp.GetReference()]=[round(x,3),round(y,3)];break
    else:raise RuntimeError('No clear reference position: '+fp.GetReference())
    # Pin one marker, placed close to pad one without covering its mask opening.
    if fp.GetReference().startswith(('J','U','X','D')):
        p=next((p for p in fp.Pads() if p.GetNumber()=='1'),None)
        if p is None:continue
        px,py=k.ToMM(p.GetPosition().x),k.ToMM(p.GetPosition().y)
        cand=[(px+dx*.2,py+dy*.2) for dx in range(-10,11) for dy in range(-10,11)]
        cand.sort(key=lambda z:(z[0]-px)**2+(z[1]-py)**2)
        for x,y in cand:
            bb=(x-.23,y-.23,x+.23,y+.23)
            if x<100.6 or x>144.4 or y<100.6 or y>121.4 or any(overlap(bb,o) for o in obstacles):continue
            s=k.PCB_SHAPE(b);s.SetShape(k.SHAPE_T_CIRCLE);s.SetCenter(k.VECTOR2I(k.FromMM(x),k.FromMM(y)));s.SetEnd(k.VECTOR2I(k.FromMM(x+.12),k.FromMM(y)));s.SetWidth(k.FromMM(.12));s.SetLayer(k.F_SilkS);b.Add(s);obstacles.append(bb);break
k.SaveBoard(str(ROOT/'imu_to_dxl.kicad_pcb'),b)
(ROOT/'source/silkscreen_positions.json').write_text(json.dumps(positions,indent=2))
print('Placed',len(positions),'references and pin-one / cathode dots')
