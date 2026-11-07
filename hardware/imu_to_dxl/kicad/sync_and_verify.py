#!/usr/bin/env python3
"""Synchronize native schematic metadata and check every source pin assignment.

Run with KiCad's bundled Python after build_kicad.py and finish_silkscreen.py.
The native CLI supplies the actual schematic connectivity; the independent
source snapshot supplies the expected pin/net assignment.
"""
from pathlib import Path
import subprocess, xml.etree.ElementTree as E, json
import pcbnew as k

ROOT=Path(__file__).resolve().parent
REPORT=ROOT/'checks';REPORT.mkdir(exist_ok=True)
CLI='/Applications/KiCad/KiCad.app/Contents/MacOS/kicad-cli'
subprocess.run([CLI,'sch','export','netlist','--format','kicadxml','-o',str(REPORT/'netlist.xml'),str(ROOT/'imu_to_dxl.kicad_sch')],check=True)
xml=E.parse(REPORT/'netlist.xml')
actual={}
for net in xml.findall('./nets/net'):
    for node in net.findall('node'):
        name=net.attrib['name']
        if name.startswith('unconnected-('):name=name.replace('/','{slash}')
        actual[node.attrib['ref'],node.attrib['pin']]=name
expected=json.loads((ROOT/'source/expected_nets.json').read_text())
bad=[]
for ref,pins in expected.items():
    for pin,want in pins.items():
        got=actual.get((ref,pin),'')
        normalized='' if got.startswith('unconnected-(') else got.removeprefix('/')
        if normalized!=want:bad.append({'reference':ref,'pin':pin,'source_net':want,'schematic_net':got})
if bad:raise RuntimeError(json.dumps(bad,indent=2))
b=k.LoadBoard(str(ROOT/'imu_to_dxl.kicad_pcb'))
# Rename each existing net once, preserving routed tracks and the ground zone.
old_to_new={}
for fp in b.GetFootprints():
    for pad in fp.Pads():
        key=(fp.GetReference(),pad.GetNumber())
        if key in actual and pad.GetNetname():old_to_new[pad.GetNetname()]=actual[key]
for old,new in old_to_new.items():
    net=b.FindNet(old)
    if net and old!=new:net.SetNetname(new)
netmap={ni.GetNetname():ni for ni in b.GetNetInfo().NetsByNetcode().values()}
compnodes={c.attrib['ref']:c for c in xml.findall('./components/comp')}
for fp in b.GetFootprints():
    ref=fp.GetReference()
    if ref not in compnodes:continue
    node=compnodes[ref]
    metadata={f.attrib['name']:f.text or '' for f in node.findall('./fields/field')}
    metadata['Datasheet']=node.findtext('datasheet','')
    for name,value in metadata.items():
        if name in ('Reference','Value','Footprint'):continue
        f=fp.GetField(name)
        if not f:
            f=k.PCB_FIELD(fp,k.FIELD_T_USER,name);fp.Add(f)
        f.SetText(value);f.SetVisible(False)
    for pad in fp.Pads():
        name=actual.get((ref,pad.GetNumber()))
        if name is None:continue
        if name not in netmap:
            ni=k.NETINFO_ITEM(b,name,b.GetNetCount()+1);b.Add(ni);netmap[name]=ni
        pad.SetNet(netmap[name])
    # Save the final per-project library, including the cleaned silkscreen geometry.
    copy=k.Cast_to_FOOTPRINT(fp.Duplicate(False));copy.SetPosition(k.VECTOR2I(0,0));copy.SetOrientationDegrees(0)
    for pad in copy.Pads():pad.SetNetCode(0)
    copy.SetPath(k.KIID_PATH())
    k.FootprintSave(str(ROOT/'imu_to_dxl.pretty'),copy)
k.SaveBoard(str(ROOT/'imu_to_dxl.kicad_pcb'),b)
result={'components':len(expected),'electrical_pins':sum(len(p) for p in expected.values()),'active_source_nets':len({v for p in expected.values() for v in p.values() if v}),'pin_net_mismatches':bad,'source': '../imu_to_dxl.eprj2','validation':'Source pin assignments versus native KiCad schematic netlist; PCB synchronized to that same netlist.'}
(REPORT/'pin_net_comparison.json').write_text(json.dumps(result,indent=2))
print(json.dumps(result,indent=2))
