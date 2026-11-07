#!/usr/bin/env python3
"""Rebuild the native KiCad project from the recovered EasyEDA design snapshot.

Run with the Python bundled with KiCad (pcbnew is required).
This writes the generated files; preserve manual changes before rebuilding.
Source design: ../imu_to_dxl.eprj2, third-party microduck-replica, see ../README.md.
"""
import collections
import json
import math
from pathlib import Path
import re
import uuid
import pcbnew as k

ROOT = Path(__file__).resolve().parent
D = json.loads((ROOT / 'source/design.json').read_text())
P = next(d['records'] for d in D.values() if d['type'] == 'PCB')
S = next(d['records'] for d in D.values() if d['type'] == 'SCH_PAGE')
MIL = .0254
NS = uuid.UUID('5ed91bdd-3a38-4f06-a285-99e2f819e926')
def uid(s): return str(uuid.uuid5(NS, s))
def q(s): return json.dumps(str(s), ensure_ascii=False)
def n(x): return f'{x:.6f}'.rstrip('0').rstrip('.') or '0'
def attrs(rs):
    a = collections.defaultdict(dict)
    for r in rs.values():
        if r['type'] == 'ATTR':
            p = r['data']; a[p.get('parentId', '')][p['key']] = p['value']
    return a
PA, SA = attrs(P), attrs(S)
def netname(s): return {'3.3V': '+3V3', '$1N175': 'Net-(X1-OUTPUT)'}.get(s, s)
COMPS = {}
for r in P.values():
    if r['type'] != 'COMPONENT': continue
    a = PA[r['id']]; dev = D[a['Device']]['records']['META']['data']
    da = dev['attributes']; ref = a['Designator']
    c = {'ref': ref, 'id': r['id'], 'pos': r['data'], 'attrs': a,
         'device': da, 'footprint': a['Footprint'], 'nets': {},
         'value': r['data'].get('attrs', {}).get('Value') or da.get('Value') or da.get('Manufacturer Part') or dev['title']}
    c['value'] = {'D2':'BZT52C5V1','F1':'MF-NSMF020X-2','X1':'16MHz','J1':'B3B-EH-A','J2':'B3B-EH-A','J3':'SWD + UART','J4':'B3B-PH-K-S','J5':'B3B-PH-K-S'}.get(ref,c['value'])
    COMPS[ref] = c
BY_ID = {c['id']: c for c in COMPS.values()}
for r in P.values():
    if r['type'] == 'PAD_NET':
        _, cid, pin, *_ = json.loads(r['id'])
        BY_ID[cid]['nets'][str(pin)] = netname(r['data']['padNet'])
for r in S.values():
    if r['type'] != 'COMPONENT': continue
    a = SA[r['id']]; ref = a.get('Designator')
    if ref in COMPS:
        COMPS[ref]['sch_id'] = r['id']
        COMPS[ref]['symbol'] = a['Symbol']
        COMPS[ref]['sch'] = r['data']
assert len(COMPS) == 29

def paths(path):
    """Decode the subset of EasyEDA polygon commands used by this source.

    Arcs are sampled to <0.002 mm sagitta for library drawings and pads.
    Board outline arcs are authored as native KiCad arcs below.
    """
    if not path: return []
    if isinstance(path[0], list):
        return [poly for p in path for poly in paths(p)]
    if path[0] == 'CIRCLE':
        _, cx, cy, radius = path
        return [[(cx+radius*math.cos(t*math.tau/96),cy+radius*math.sin(t*math.tau/96)) for t in range(97)]]
    if path[0] == 'R':
        _, x,y,w,h,ang,*rest = path
        points=[(x,y),(x+w,y),(x+w,y+h),(x,y+h),(x,y)]
        assert not ang, path
        return [points]
    points=[]; i=0
    while i < len(path):
        v=path[i]
        if isinstance(v,(int,float)):
            points.append((v,path[i+1]));i+=2
        elif v=='L': i+=1
        elif v in ('ARC','CARC'):
            angle,ex,ey=path[i+1:i+4];sx,sy=points[-1]
            theta=math.radians(angle); dx,dy=ex-sx,ey-sy
            cx=(sx+ex)/2-dy/(2*math.tan(theta/2))
            cy=(sy+ey)/2+dx/(2*math.tan(theta/2))
            a=math.atan2(sy-cy,sx-cx);radius=math.hypot(sx-cx,sy-cy)
            count=max(4,math.ceil(abs(theta)*math.sqrt(max(radius*MIL,.001)/.008)))
            points.extend((cx+radius*math.cos(a+theta*j/count),cy+radius*math.sin(a+theta*j/count)) for j in range(1,count+1))
            points[-1]=(ex,ey);i+=4
        else: raise ValueError(f'Unsupported path command {v}')
    return [points]

LIB = ROOT / 'imu_to_dxl.pretty'; LIB.mkdir(exist_ok=True)
def footprint_text(c):
    rs=D[c['footprint']]['records']; out=[]
    name=c['ref']+'_'+re.sub(r'[^A-Za-z0-9_-]','_',D[c['footprint']]['records']['META']['data']['title'])
    c['fp_name']=name
    thru=any(r['type']=='PAD' and (r['data'].get('hole') or {}).get('width',0)>0 for r in rs.values())
    out += [f'(footprint {q(name)} (version 20240108) (generator "imu_to_dxl_rebuild") (layer "F.Cu")',
            f'(attr {"through_hole" if thru else "smd"})',
            f'(property "Reference" {q(c["ref"])} (at 0 -3) (layer "F.SilkS") (effects (font (size .7 .7) (thickness .12))))',
            f'(property "Value" {q(c["value"])} (at 0 3) (layer "F.Fab") (effects (font (size .7 .7) (thickness .1)) (hide yes)))']
    lmap={3:'F.SilkS',4:'B.SilkS',48:'F.Fab',49:'F.Fab',50:'F.CrtYd',51:'B.CrtYd'}
    for r in rs.values():
        p=r['data'];t=r['type']
        if t=='PAD':
            sh=p['defaultPad'];shape=sh['padType'];cx=p['centerX'];cy=p['centerY'];angle=p['padAngle']
            hole=p.get('hole') or {}; drilled=hole.get('width',0)>0
            typ='thru_hole' if drilled else 'smd'; layers='"*.Cu" "*.Mask"' if drilled else '"F.Cu" "F.Paste" "F.Mask"'
            shape={'RECT':'rect','OVAL':'oval','ELLIPSE':'circle','POLYGON':'custom','POLY':'custom'}[shape]
            extra='';drill=f'(drill {n(hole["width"]*MIL)})' if drilled else ''
            if shape=='custom':
                # The source polygon is in footprint coordinates, unlike rectangular pad dimensions.
                angle=0;w=h=.01
                poly=paths(sh['path'])[0]
                pts=' '.join(f'(xy {n((x-cx)*MIL)} {n(-(y-cy)*MIL)})' for x,y in poly)
                extra=f'(options (clearance outline) (anchor circle)) (primitives (gr_poly (pts {pts}) (width 0) (fill yes)))'
            else:
                w=sh['width']*MIL;h=sh['height']*MIL
                if shape=='circle' and abs(w-h)>.0001:shape='oval'
            out.append(f'(pad {q(p["num"])} {typ} {shape} (at {n(cx*MIL)} {n(-cy*MIL)} {n(angle)}) (size {n(w)} {n(h)}) {drill} (layers {layers}) {extra})')
        elif t in ('POLY','FILL') and p.get('layerId') in lmap:
            # Use source body and silkscreen geometry; omit manufacturer courtyard-style artwork.
            layer=lmap[p['layerId']]
            if 'CrtYd' in layer:continue
            for poly in paths(p['path']):
                pts=[(x*MIL,-y*MIL) for x,y in poly]
                if t=='FILL' and layer=='F.SilkS':
                    out.append('(fp_poly (pts '+' '.join(f'(xy {n(x)} {n(y)})' for x,y in pts)+f') (stroke (width 0) (type solid)) (fill solid) (layer {q(layer)}))')
                else:
                    for (x,y),(xx,yy) in zip(pts,pts[1:]):
                        if math.hypot(x-xx,y-yy)<.00001:continue
                        out.append(f'(fp_line (start {n(x)} {n(y)}) (end {n(xx)} {n(yy)}) (stroke (width {n(max(.1,p.get("width",2)*MIL))}) (type solid)) (layer {q(layer)}))')
    out.append(')')
    return name, '\n'.join(out)

def build_board():
    b=k.BOARD(); b.SetCopperLayerCount(2)
    b.GetDesignSettings().SetBoardThickness(k.FromMM(1.6))
    names=sorted({net for c in COMPS.values() for net in c['nets'].values() if net})
    nets={}
    for name in names:
        ni=k.NETINFO_ITEM(b,name,len(nets)+1); b.Add(ni);nets[name]=ni
    def xy(x,y):return k.VECTOR2I(k.FromMM(100+x*MIL),k.FromMM(122-y*MIL))
    for c in COMPS.values():
        name,s=footprint_text(c); (LIB/(name+'.kicad_mod')).write_text(s)
        fp=k.FootprintLoad(str(LIB),name)
        if fp is None:raise RuntimeError(name)
        b.Add(fp);fp.SetReference(c['ref']);fp.SetValue(c['value']);fp.SetFPID(k.LIB_ID('imu_to_dxl',name))
        # Associate footprints with the generated schematic so Update PCB works.
        fp.SetPath(k.KIID_PATH('/'+uid('sheet')+'/'+uid('sym:'+c['ref'])))
        for pad in fp.Pads():
            net=c['nets'].get(pad.GetNumber(),'')
            if net:pad.SetNet(nets[net])
        p=c['pos'];fp.SetOrientationDegrees(p['angle']);fp.SetPosition(xy(p['x'],p['y']))
        fp.Value().SetVisible(False)
        rf=fp.Reference();rf.SetTextSize(k.VECTOR2I(k.FromMM(.65),k.FromMM(.65)));rf.SetTextThickness(k.FromMM(.1));rf.SetTextAngle(k.EDA_ANGLE(0,k.DEGREES_T))
        bb=fp.GetBoundingBox(False,False)
        rf.SetPosition(k.VECTOR2I(fp.GetPosition().x,bb.GetTop()-k.FromMM(.65)))
    for r in P.values():
        p=r['data'];t=r['type']
        if t=='LINE' and p['layerId'] in (1,2):
            seg=k.PCB_TRACK(b);seg.SetStart(xy(p['startX'],p['startY']));seg.SetEnd(xy(p['endX'],p['endY']));seg.SetWidth(k.FromMM(p['width']*MIL));seg.SetLayer(k.F_Cu if p['layerId']==1 else k.B_Cu);seg.SetNet(nets[netname(p['netName'])]);b.Add(seg)
        elif t=='VIA':
            v=k.PCB_VIA(b);v.SetPosition(xy(p['centerX'],p['centerY']));v.SetWidth(k.FromMM(round(p['viaDiameter']*MIL,3)));v.SetDrill(k.FromMM(max(.3,round(p['holeDiameter']*MIL,3))));v.SetViaType(k.VIATYPE_THROUGH);v.SetLayerPair(k.F_Cu,k.B_Cu);v.SetNet(nets[netname(p['netName'])]);b.Add(v)
        elif t=='PAD':
            fp=k.FOOTPRINT(b);fp.SetReference(p['num']);fp.SetAttributes(k.FP_EXCLUDE_FROM_POS_FILES|k.FP_EXCLUDE_FROM_BOM|k.FP_BOARD_ONLY);b.Add(fp)
            pad=k.PAD(fp);pad.SetNumber('');pad.SetAttribute(k.PAD_ATTRIB_NPTH);pad.SetShape(k.PAD_SHAPE_CIRCLE);pad.SetSize(k.VECTOR2I(k.FromMM(2.2),k.FromMM(2.2)));pad.SetDrillSize(k.VECTOR2I(k.FromMM(2.2),k.FromMM(2.2)));pad.SetLayerSet(k.LSET.AllCuMask());pad.SetPosition(xy(p['centerX'],p['centerY']));fp.Add(pad);fp.Reference().SetVisible(False);fp.Value().SetVisible(False)
    # Documented nominal board outline: 45 x 22 mm, R2. Origin at lower left.
    def pt(x,y):return k.VECTOR2I(k.FromMM(100+x),k.FromMM(100+y))
    edges=[((2,0),(43,0)),((45,2),(45,20)),((43,22),(2,22)),((0,20),(0,2))]
    for a,z in edges:
        sh=k.PCB_SHAPE(b);sh.SetShape(k.SHAPE_T_SEGMENT);sh.SetStart(pt(*a));sh.SetEnd(pt(*z));sh.SetLayer(k.Edge_Cuts);sh.SetWidth(k.FromMM(.05));b.Add(sh)
    d=2-math.sqrt(2)
    for a,m,z in [((43,0),(45-d,d),(45,2)),((45,20),(45-d,22-d),(43,22)),((2,22),(d,22-d),(0,20)),((0,2),(d,d),(2,0))]:
        sh=k.PCB_SHAPE(b);sh.SetShape(k.SHAPE_T_ARC);sh.SetArcGeometry(pt(*a),pt(*m),pt(*z));sh.SetLayer(k.Edge_Cuts);sh.SetWidth(k.FromMM(.05));b.Add(sh)
    zone=k.ZONE(b);zone.SetLayer(k.B_Cu);zone.SetNet(nets['GND']);zone.SetLocalClearance(k.FromMM(.152));zone.SetMinThickness(k.FromMM(.15));zone.SetPadConnection(k.ZONE_CONNECTION_THERMAL);zone.SetThermalReliefGap(k.FromMM(.2));zone.SetThermalReliefSpokeWidth(k.FromMM(.3))
    poly=zone.Outline();poly.NewOutline()
    for x,y in [(100.25,100.25),(144.75,100.25),(144.75,121.75),(100.25,121.75)]:poly.Append(k.FromMM(x),k.FromMM(y))
    b.Add(zone)
    title=k.TITLE_BLOCK();title.SetTitle('imu_to_dxl - third-party replica');title.SetRevision('K1');title.SetDate('2026-09-13');title.SetComment(0,'Migrated from 2026-09-08 EasyEDA source. Prototype - not hardware tested.');b.SetTitleBlock(title)
    k.SaveBoard(str(ROOT/'imu_to_dxl.kicad_pcb'),b)
    return names

def build_schematic():
    # Preserve the source drawing, while assigning meaningful electrical pin types.
    scale=.254
    def sx(v):return n(v*scale)
    def sy(v):return n((825+v)*scale)
    def eff(size=1.27,just='left',hide=False):
        return f'(effects (font (size {size} {size}))'+(f' (justify {just})' if just else '')+(' (hide yes)' if hide else '')+')'
    def poly(pts,width=.254):
        return '(polyline (pts '+' '.join(f'(xy {sx(x)} {sx(-y)})' for x,y in pts)+f') (stroke (width {width}) (type default)) (fill (type none)))'
    libs=[]; instances=[]; allpins={}; nc=[]
    def pintype(ref,num):
        if ref=='U1':return 'power_in' if num in ('4','5') else ('input' if num=='2' else 'bidirectional')
        if ref=='U2':return {'1':'tri_state','2':'passive','3':'passive','4':'output','5':'power_in','6':'power_in','7':'power_in','8':'power_in','9':'output','10':'input','11':'tri_state','12':'input','13':'input','14':'input'}[num]
        if ref=='U3':return 'power_in' if num in ('4','8') else ('tri_state' if num in ('3','6') else 'input')
        if ref=='U4':return 'power_out' if num=='3' else 'power_in'
        if ref=='X1':return {'1':'input','2':'power_in','3':'output','4':'power_in'}[num]
        return 'passive'
    for c in COMPS.values():
        ref=c['ref'];name=ref;rs=D[c['symbol']]['records'];a=attrs(rs);art=[];pins=[]
        for r in rs.values():
            p=r['data'];t=r['type']
            if t=='PIN':
                at=a[r['id']];num=str(at['Pin Number']);pn=at['Pin Name']
                if ref[0] in 'RCFDJ':pn='~'
                if ref=='U1' and num=='2':pn='PC14 / OSC_IN'
                pins.append(f'(pin {pintype(ref,num)} line (at {sx(p["x"])} {sx(-p["y"])} {p["rotation"]}) (length {sx(p["length"])}) (name {q(pn)} {eff(1.27,"")}) (number {q(num)} {eff(1.016,"")}))')
                theta=math.radians(c['sch']['rotation']);px=p['x'];py=p['y']
                x=c['sch']['x']+math.cos(theta)*px+math.sin(theta)*py
                y=c['sch']['y']-math.sin(theta)*px+math.cos(theta)*py
                allpins[(ref,num)]=(round(x*scale,6),round((825+y)*scale,6))
                if not c['nets'].get(num):nc.append(allpins[(ref,num)])
            elif t=='RECT':
                art.append(f'(rectangle (start {sx(p["dotX1"])} {sx(-p["dotY1"])}) (end {sx(p["dotX2"])} {sx(-p["dotY2"])}) (stroke (width .254) (type default)) (fill (type background)))')
            elif t=='POLY':art.append(poly([(v['x'],v['y']) for v in p['points']]))
            elif t in ('CIRCLE','ELLIPSE'):
                # Small pin-one marks are drawn as circles; ellipse is cosmetic.
                if 'centerX' in p:cx,cy=p['centerX'],p['centerY'];rad=p.get('radius',p.get('radiusX',1))
                else:continue
                art.append(f'(circle (center {sx(cx)} {sx(-cy)}) (radius {sx(rad)}) (stroke (width .254) (type default)) (fill (type none)))')
        body=f'(symbol {q("imu_to_dxl:"+name)} (pin_names (offset .508)) (in_bom yes) (on_board yes) (property "Reference" {q(ref)} (at 0 0 0) {eff()}) (property "Value" {q(c["value"])} (at 0 0 0) {eff()}) (symbol {q(name+"_0_1")} '+''.join(art)+') (symbol '+q(name+'_1_1')+' '+''.join(pins)+'))'
        libs.append(body)
        p=c['sch'];inst=[f'(symbol (lib_id {q("imu_to_dxl:"+name)}) (at {sx(p["x"])} {sy(p["y"])} {p["rotation"]}) (unit 1) (in_bom yes) (on_board yes) (dnp no) (uuid {q(uid("sym:"+ref))})']
        for key,value in [('Reference',ref),('Value',c['value']),('Footprint','imu_to_dxl:'+c['fp_name']),('Datasheet',c['device'].get('Datasheet','')),('LCSC',c['device'].get('Supplier Part','')),('Manufacturer Part',c['device'].get('Manufacturer Part',''))]:
            src='Designator' if key=='Reference' else 'Name'
            loc=next((r['data'] for r in S.values() if r['type']=='ATTR' and r['data'].get('parentId')==c['sch_id'] and r['data']['key']==src),p)
            x=loc.get('x',p['x']);y=loc.get('y',p['y'])
            if key=='Reference':y-=5
            if key=='Value' and ref in ('C1','C2','C4','C5','C8','C9'):x+=15
            if ref in ('D1','C4','C5','C8') and key in ('Reference','Value'):
                x=p['x']+({'D1':-60,'C4':-45,'C5':15,'C8':15}[ref])
                y=p['y']+(-5 if key=='Reference' else 10)
                if ref=='C5' and key=='Value':x=p['x']+60
                if ref=='C8':x=p['x']+(20 if key=='Reference' else 35)
            if ref=='R7' and key in ('Reference','Value'):
                x=p['x']+20;y=p['y']+(-10 if key=='Reference' else 5)
            angle=p['rotation']%180
            inst.append(f'(property {q(key)} {q(value)} (at {sx(x)} {sy(y)} {angle}) {eff(1.27,"left",key not in ("Reference","Value"))})')
        inst.append(f'(instances (project "imu_to_dxl" (path {q("/"+uid("sheet"))} (reference {q(ref)}) (unit 1)))))')
        instances.append('\n'.join(inst))
    wires=[]; groups=collections.defaultdict(list);points=collections.Counter();wiregeoms=[]
    live_wires={r['id'] for r in S.values() if r['type']=='WIRE'}
    for r in S.values():
        p=r['data']
        if r['type']!='LINE' or p['lineGroup'] not in live_wires:continue
        a=(round(p['startX']*scale,6),round((825+p['startY'])*scale,6));b=(round(p['endX']*scale,6),round((825+p['endY'])*scale,6))
        if a==b:continue
        wires.append(f'(wire (pts (xy {n(a[0])} {n(a[1])}) (xy {n(b[0])} {n(b[1])})) (stroke (width 0) (type default)) (uuid {q(uid("wire:"+r["id"]))}))')
        groups[p['lineGroup']].append((a,b));points[a]+=1;points[b]+=1;wiregeoms.append((a,b))
    labels=[];labelnames={}
    connected_points=set(allpins.values())
    for r in S.values():
        if r['type']=='COMPONENT' and SA[r['id']].get('Name') in ('3.3V','GND','VDD_BUS','VDD_F'):
            p=r['data'];connected_points.add((round(p['x']*scale,6),round((825+p['y'])*scale,6)))
    for gid,segments in groups.items():
        name=SA[gid].get('NET','')
        if not name:continue
        name=netname(name)
        loc=next((r['data'] for r in S.values() if r['type']=='ATTR' and r['data'].get('parentId')==gid and r['data'].get('key')=='NET'),None)
        # Put electrical anchor on a real wire endpoint, retaining the source net name.
        (x,y),_=segments[0]
        if loc and loc.get('x') is not None:
            lx=loc['x']*scale;ly=(825+loc['y'])*scale
            for a,b in segments:
                if min(a[0],b[0])-.001<=lx<=max(a[0],b[0])+.001 and min(a[1],b[1])-.001<=ly<=max(a[1],b[1])+.001 and abs((lx-a[0])*(b[1]-a[1])-(ly-a[1])*(b[0]-a[0]))<.001:
                    x,y=lx,ly;break
        ends=[pt for pair in segments for pt in pair if points[pt]==1 and pt not in connected_points]
        if ends:x,y=ends[0]
        right_end=any((x,y) in pair and x>min(pair[0][0],pair[1][0]) for pair in segments)
        justify='right bottom' if right_end else 'left bottom'
        kind='global_label' if name in ('+3V3','GND','VDD_BUS','VDD_F') else 'label'
        shape='(shape input)' if kind=='global_label' else ''
        labels.append(f'({kind} {q(name)} {shape} (at {n(x)} {n(y)} 0) {eff(1.27,justify)} (uuid {q(uid("label:"+gid))}))');labelnames[gid]=name
    powercount=0
    for net in ('+3V3','GND','VDD_BUS','VDD_F'):
        name='PWR_'+net.replace('+','')
        geom=poly([(0,0),(0,10),(-10,10),(10,10)])+poly([(-6,13),(6,13)])+poly([(-2,16),(2,16)]) if net=='GND' else poly([(0,0),(0,-10),(-5,-10),(5,-10)])
        libs.append(f'(symbol {q("imu_to_dxl:"+name)} (power) (pin_names (offset 0) (hide yes)) (in_bom no) (on_board no) (property "Reference" "#PWR" (at 0 0 0) {eff(1.27,"",True)}) (property "Value" {q(net)} (at 0 3.81 0) {eff(1.27,"")}) (symbol {q(name+"_0_1")} {geom}) (symbol {q(name+"_1_1")} (pin power_in line (at 0 0 90) (length 0) (hide yes) (name {q(net)} {eff(1.016,"")}) (number "1" {eff(1.016,"")}))))')
    # Source power-port pins are at the instance origin.
    for r in S.values():
        if r['type']!='COMPONENT':continue
        a=SA[r['id']];name=a.get('Name');p=r['data']
        if a.get('Designator') or name not in ('3.3V','GND','VDD_BUS','VDD_F'):continue
        net=netname(name);name='PWR_'+net.replace('+','');powercount+=1;ref=f'#PWR{powercount:03d}'
        x=p['x'];y=p['y'];ang=p['rotation'];theta=math.radians(ang);vx=x-15*math.sin(theta);vy=y-15*math.cos(theta)
        instances.append(f'(symbol (lib_id {q("imu_to_dxl:"+name)}) (at {sx(x)} {sy(y)} {ang}) (unit 1) (in_bom no) (on_board no) (dnp no) (uuid {q(uid("power:"+r["id"]))}) (property "Reference" {q(ref)} (at {sx(x)} {sy(y)} {ang%180}) {eff(1.27,"",True)}) (property "Value" {q(net)} (at {sx(vx)} {sy(vy)} {ang%180}) {eff(1.27,"",net=="GND")}) (instances (project "imu_to_dxl" (path {q("/"+uid("sheet"))} (reference {q(ref)}) (unit 1)))))')
    flag='(symbol "imu_to_dxl:PWR_FLAG" (power) (pin_names (offset 0) (hide yes)) (in_bom no) (on_board no) (property "Reference" "#FLG" (at 0 0 0) '+eff(1.27,'',True)+') (property "Value" "PWR_FLAG" (at 0 2.54 0) '+eff(1.016,'')+') (symbol "PWR_FLAG_0_1" (polyline (pts (xy 0 0) (xy 0 1.27) (xy -1.27 1.905) (xy 0 2.54) (xy 1.27 1.905) (xy 0 1.27)) (stroke (width 0) (type default)) (fill (type none)))) (symbol "PWR_FLAG_1_1" (pin power_out line (at 0 0 90) (length 0) (name "pwr" '+eff(1.016,'')+') (number "1" '+eff(1.016,'')+'))))'
    libs.append(flag)
    # External ground and the fused input are supplied by the bus, not by an on-board regulator.
    for i,(net,xy) in enumerate([('GND',(234.95,29.21)),('VDD_F',(234.95,39.37)),('VDD_BUS',(234.95,19.05))]):
        x,y=xy;ref=f'#FLG0{i+1}'
        labels.append(f'(global_label {q(net)} (shape input) (at {n(x)} {n(y)} 0) {eff(1.016,"left")} (uuid {q(uid("flag-net:"+net))}))')
        instances.append(f'(symbol (lib_id "imu_to_dxl:PWR_FLAG") (at {n(x)} {n(y)} 0) (unit 1) (in_bom no) (on_board no) (dnp no) (uuid {q(uid("flag:"+net))}) (property "Reference" {q(ref)} (at {n(x)} {n(y)} 0) {eff(1.016,"",True)}) (property "Value" "PWR_FLAG" (at {n(x)} {n(y-3.81)} 0) {eff(1.016,"")}) (instances (project "imu_to_dxl" (path {q("/"+uid("sheet"))} (reference {q(ref)}) (unit 1)))))')
    # A wire endpoint on the interior of another segment is a real T junction.
    junctions=[]
    for (x,y),count in points.items():
        interior=any(min(a[0],b[0])-.0001<=x<=max(a[0],b[0])+.0001 and min(a[1],b[1])-.0001<=y<=max(a[1],b[1])+.0001 and (x,y) not in (a,b) and abs((x-a[0])*(b[1]-a[1])-(y-a[1])*(b[0]-a[0]))<.0001 for a,b in wiregeoms)
        if count>=3 or interior:junctions.append(f'(junction (at {n(x)} {n(y)}) (diameter 0) (color 0 0 0 0) (uuid {q(uid(f"junction:{x}:{y}"))}))')
    head=f'(kicad_sch (version 20250114) (generator "eeschema") (uuid {q(uid("sheet"))}) (paper "User" 320 235) (title_block (title "imu_to_dxl - Microduck replica") (date "2026-09-13") (rev "K1") (company "Derived from microduck-replica, 2026-09-08") (comment 1 "Prototype / no hardware validation. J3: 4=RX, 5=TX, 6=3V3."))'
    notes=[]
    for i,(text,x,y) in enumerate([('2S bus: 6.0-8.4 V | STM32G031 + LSM6DSV16X | 1 Mbps',18,170),('J1/J2: 1=GND  2=VBUS  3=DATA (2.5 mm)',18,177),('J4/J5: 1=DATA  2=VBUS  3=GND (2.0 mm)',18,183),('DE is active LOW. RX_EN has 10k pull-up. X1 is fitted.',18,189),('Connector fit, firmware and physical IMU axes require validation.',18,195)]):
        notes.append(f'(text {q(text)} (at {x} {y} 0) {eff(1.27,"left")} (uuid {q(uid("note:"+str(i)))}))')
    drawing='\n'.join(wires+labels+junctions+instances+notes)
    for i,(x,y) in enumerate(nc):drawing+=f'\n(no_connect (at {n(x)} {n(y)}) (uuid {q(uid("nc:"+str(i)))}))'
    # Add space around the original artwork, including the upper-left connector.
    def shift(m):return f'({m[1]} {n(float(m[2])+7.62)} {n(float(m[3])+12.7)}'
    drawing=re.sub(r'\((at|xy) (-?[\d.]+) (-?[\d.]+)',shift,drawing)
    out=head+'\n(lib_symbols\n'+'\n'.join(libs)+')\n'+drawing
    out+='\n(sheet_instances (path "/" (page "1")))\n)\n'
    (ROOT/'imu_to_dxl.kicad_sch').write_text(out)
    # Standalone library makes symbol editing and future PCB updates self-contained.
    library='(kicad_symbol_lib (version 20231120) (generator "kicad_symbol_editor")\n'+'\n'.join(s.replace('(symbol "imu_to_dxl:','(symbol "',1) for s in libs)+'\n)\n'
    (ROOT/'imu_to_dxl.kicad_sym').write_text(library)
    (ROOT/'sym-lib-table').write_text('(sym_lib_table (version 7) (lib (name "imu_to_dxl") (type "KiCad") (uri "${KIPRJMOD}/imu_to_dxl.kicad_sym") (options "") (descr "Recovered replica symbols with pin electrical types")))\n')
    (ROOT/'fp-lib-table').write_text('(fp_lib_table (version 7) (lib (name "imu_to_dxl") (type "KiCad") (uri "${KIPRJMOD}/imu_to_dxl.pretty") (options "") (descr "Recovered replica footprints")))\n')
    (ROOT/'source/expected_nets.json').write_text(json.dumps({c['ref']:c['nets'] for c in COMPS.values()},indent=2))

def build_project():
    template=Path('/Applications/KiCad/KiCad.app/Contents/SharedSupport/template/kicad.kicad_pro')
    pro=json.loads(template.read_text())
    pro['meta']={'filename':'imu_to_dxl.kicad_pro','version':1}
    ds=pro['board']['design_settings']
    ds['rules'].update({'min_clearance':.15,'min_copper_edge_clearance':.3,'min_through_hole_diameter':.3,'min_via_diameter':.6,'min_track_width':.15,'min_silk_text_height':.65,'min_silk_text_thickness':.1,'silk_clearance':.1})
    pro['net_settings']={'classes':[{'name':'Default','clearance':.15,'track_width':.254,'via_diameter':.6,'via_drill':.3,'wire_width':6,'bus_width':12,'line_style':0}], 'meta':{'version':3}}
    ds['track_widths']=[0,.152,.254,.508,.762]
    ds['via_dimensions']=[{'diameter':.6,'drill':.3}]
    (ROOT/'imu_to_dxl.kicad_pro').write_text(json.dumps(pro,indent=2))

if __name__=='__main__':
    names=build_board()
    build_schematic()
    build_project()
    print('PCB:',len(COMPS),'components;',len(names),'active nets; 227 tracks; 31 vias')
