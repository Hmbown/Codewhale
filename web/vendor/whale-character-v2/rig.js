// The mark is the rig. Profile contours are partitioned from the supplied SVG,
// retaining its cubic anchors. Every pose retains each part's command topology.
(function (root) {
  'use strict';
  const TAU = Math.PI * 2, D2R = Math.PI / 180;
  const lerp = (a,b,t) => a+(b-a)*t;
  const clamp = (v,a,b) => Math.max(a,Math.min(b,v));
  const smooth = (a,b,v) => { const t=clamp((v-a)/(b-a),0,1); return t*t*(3-2*t); };
  const POSE = {x:0,y:0,rot:0,scale:1,squash:0,tilt:0,yaw:0,curl:0,arch:0,head:0,
    fluke:6,flukeSpread:1,fin:0,finFar:0,lookX:0,lookY:0,lid:0,lidLow:0,eyeScale:1,
    brow:0,browTilt:0,mouth:0,smile:.4,mouthSide:0};
  const DIRECTIONS = {
    mark:{key:'mark',name:'01 / Emblem',uncurl:0,frontWidth:1},
    cruise:{key:'cruise',name:'02 / Cruise',uncurl:.56,frontWidth:1.08},
    open:{key:'open',name:'03 / Open C',uncurl:.25,frontWidth:.94}
  };
  DIRECTIONS.calf=DIRECTIONS.mark; // Compatibility with gallery.html#dir=calf.
  const [outer,pouch,eye,pleat1,pleat2]=root.WhaleMark.contours;
  const copy=x=>JSON.parse(JSON.stringify(x));
  const end=c=>c.slice(-2);
  // The pointed blade below the white wedge is the flipper. The rounded lobe
  // above that wedge is the body, and must never rotate off with the arm.
  const seam=['C',250,405,273,445,262.32,471.04];
  // The far-side blade stays tucked in the mark. The near blade lifts from it.
  // Keeping the body beneath that joint also keeps the jaw band continuous.
  const body=[...copy(outer.slice(0,125)),['C',215,92,218,59,215.77,40.96],['Z']];
  const flipper=[['M',228.68,388.82],...copy(outer.slice(48,54)),['C',273,445,250,405,228.68,388.82],['Z']];
  const flukes=[['M',203.32,110.65],...copy(outer.slice(125,-1)),['Z']];
  const indexAt=(path,x,y)=>path.findIndex(c=>c.at(-2)===x&&c.at(-1)===y);
  const ji=indexAt(outer,141.12,447.47), je=indexAt(outer,496,236.55), pe=indexAt(pouch,238.49,455.19);
  const jaw=[['M',141.12,447.47],...copy(outer.slice(ji+1,je+1)),['C',497,249,486,265,483.42,270.06],
    ...copy(pouch.slice(3,pe+1)),['C',209,463,169,458,141.12,447.47],['Z']];
  const parse=d=>{const t=d.match(/[MCZ]|-?\d*\.?\d+/g),p=[];for(let i=0;i<t.length;){const op=t[i++],n={M:2,C:6,Z:0}[op];p.push([op,...t.slice(i,i+n).map(Number)]);i+=n;}return p;};
  const frontBody=parse('M244 70 C147 65 78 114 76 228 C75 279 76 303 94 342 C112 370 127 380 151 378 C172 450 250 475 286 465 C384 453 436 399 442 341 C442 320 440 310 431 310 C353 269 270 274 215 288 C176 300 138 314 121 294 C101 269 112 222 127 192 C153 145 203 127 244 128 C244 108 244 87 244 70 Z');
  const frontPouch=parse('M416 353 C405 398 362 431 288 443 C234 448 184 410 155 352 C204 368 241 374 281 374 C328 374 375 365 416 353 Z');
  const frontFlukes=parse('M244 128 C270 130 274 154 302 151 C324 149 343 126 346 108 C321 107 314 94 302 100 C279 95 263 97 250 96 C265 86 282 67 285 52 C267 49 249 64 244 70 C244 96 244 116 244 128 Z');
  const frontFin=parse('M165 360 C147 376 139 397 124 404 C147 408 163 402 180 390 C183 378 174 367 165 360 Z');
  const frontJaw=parse('M155 352 C184 410 234 448 288 443 C362 431 405 398 416 353 C424 354 435 350 442 341 C436 399 384 453 286 465 C230 470 180 435 151 378 C150 368 152 359 155 352 Z');
  const frontPleat1=parse('M330 384 C329 405 315 424 294 434 C314 430 336 405 330 384 Z');
  const frontPleat2=parse('M247 386 C247 404 261 423 277 433 C257 426 242 403 247 386 Z');
  function ellipse(x,y,rx,ry){const k=.55228475;return [['M',x+rx,y],['C',x+rx,y+ry*k,x+rx*k,y+ry,x,y+ry],['C',x-rx*k,y+ry,x-rx,y+ry*k,x-rx,y],['C',x-rx,y-ry*k,x-rx*k,y-ry,x,y-ry],['C',x+rx*k,y-ry,x+rx,y-ry*k,x+rx,y],['Z']];}
  // Resample only the authored front target to match the exact source topology.
  // Shared Catmull tangents make the target C1-continuous at every anchor.
  function points(path){let p=[],last;for(const c of path){if(c[0]==='M'){last=c.slice(1);p.push(last);}else if(c[0]==='C'){let a=last;for(let j=1;j<=10;j++){let t=j/10,q=1-t;p.push([q*q*q*a[0]+3*q*q*t*c[1]+3*q*t*t*c[3]+t*t*t*c[5],q*q*q*a[1]+3*q*q*t*c[2]+3*q*t*t*c[4]+t*t*t*c[6]]);}last=c.slice(-2);}}return p;}
  function measure(path){let start=null,last=null,length=0,knots=[],samples=[];
    for(const c of path){if(c[0]==='M'){start=last=c.slice(1);samples.push({l:0,p:last,d:[1,0]});}
      else if(c[0]==='C'){const a=last,l0=length;let prev=a;
        for(let j=1;j<=30;j++){const t=j/30,q=1-t;
          const p=[q*q*q*a[0]+3*q*q*t*c[1]+3*q*t*t*c[3]+t*t*t*c[5],q*q*q*a[1]+3*q*q*t*c[2]+3*q*t*t*c[4]+t*t*t*c[6]];
          const d=[3*q*q*(c[1]-a[0])+6*q*t*(c[3]-c[1])+3*t*t*(c[5]-c[3]),3*q*q*(c[2]-a[1])+6*q*t*(c[4]-c[2])+3*t*t*(c[6]-c[4])];
          length+=Math.hypot(p[0]-prev[0],p[1]-prev[1]);samples.push({l:length,p,d});prev=p;
        }
        const seg=length-l0;knots.push({l0,l1:length,h0:Math.hypot(c[1]-a[0],c[2]-a[1])/(seg||1),h1:Math.hypot(c[3]-c[5],c[4]-c[6])/(seg||1)});last=c.slice(-2);
      }}samples[0].d=samples[1].d;return {length,knots,samples};}
  function match(source,target){const src=measure(source),dst=measure(target);
    const at=u=>{const l=clamp(u,0,1)*dst.length;let i=1;while(i<dst.samples.length-1&&dst.samples[i].l<l)i++;
      const a=dst.samples[i-1],b=dst.samples[i],t=(l-a.l)/(b.l-a.l||1),d=[lerp(a.d[0],b.d[0],t),lerp(a.d[1],b.d[1],t)],n=Math.hypot(...d)||1;
      return {p:[lerp(a.p[0],b.p[0],t),lerp(a.p[1],b.p[1],t)],d:[d[0]/n,d[1]/n]};};
    const out=[['M',...at(0).p]];
    for(const k of src.knots){const a=at(k.l0/src.length),b=at(k.l1/src.length),span=(k.l1-k.l0)/src.length*dst.length,h0=span*Math.min(.6,k.h0),h1=span*Math.min(.6,k.h1);
      out.push(['C',a.p[0]+a.d[0]*h0,a.p[1]+a.d[1]*h0,b.p[0]-b.d[0]*h1,b.p[1]-b.d[1]*h1,...b.p]);}
    out.push(['Z']);return out;}

  const SRC={body,pouch,'pleat-1':pleat1,'pleat-2':pleat2,'jaw-band':jaw,eye,eyelid:ellipse(294,337,18,15),flipper,flukes,'eye-far':copy(eye)};
  const TARGET={body:frontBody,pouch:frontPouch,'pleat-1':frontPleat1,'pleat-2':frontPleat2,'jaw-band':frontJaw,
    eye:ellipse(200,330,13,8),eyelid:ellipse(200,330,18,15),flipper:frontFin,flukes:frontFlukes,'eye-far':ellipse(365,330,13,8)};
  for(const path of Object.values(SRC)){const first=path[0].slice(1),last=path.at(-2).slice(-2);if(Math.hypot(first[0]-last[0],first[1]-last[1])>.01)path.splice(-1,0,['C',lerp(last[0],first[0],1/3),lerp(last[1],first[1],1/3),lerp(last[0],first[0],2/3),lerp(last[1],first[1],2/3),...first]);}
  const signedArea=path=>{const p=points(path);return p.reduce((sum,a,i)=>{const b=p[(i+1)%p.length];return sum+a[0]*b[1]-b[0]*a[1];},0)/2;};
  function orientLike(source,path){if(signedArea(source)*signedArea(path)>=0)return path;
    const segments=[];let at=path[0].slice(1),start=at;
    for(const c of path)if(c[0]==='C'){segments.push({at,c});at=c.slice(-2);}
    if(Math.hypot(at[0]-start[0],at[1]-start[1])>.01)segments.push({at,c:['C',...at,...start,...start]});
    return [['M',...start],...segments.reverse().map(({at,c})=>['C',c[3],c[4],c[1],c[2],...at]),['Z']];
  }
  const FRONT=Object.fromEntries(Object.keys(SRC).map(k=>[k,match(SRC[k],orientLike(SRC[k],TARGET[k]))]));
  // Match anatomical landmarks, not percentages around the entire perimeter.
  // Otherwise the short snout turns into a corner and the belly migrates upward.
  function pieces(source,stops,targets){let begin=0,result=[];
    for(let i=0;i<stops.length;i++){
      const last=stops[i],section=begin===0?source.slice(0,last+1):[['M',...end(source[begin])],...source.slice(begin+1,last+1)];
      const fitted=match(section,parse(targets[i]));result.push(...fitted.slice(i?1:0,-1));begin=last;
    }return [...result,['Z']];}
  FRONT.body=pieces(SRC.body,[47,53,80,124,125],[
    'M244 70 C147 65 78 114 76 228 C75 279 98 325 128 335 C142 347 144 363 151 378',
    'M151 378 C164 410 187 434 210 449 C231 462 261 468 286 465',
    'M286 465 C384 453 436 399 442 341 C442 320 440 310 431 310',
    'M431 310 C353 269 270 274 215 288 C176 300 138 314 121 294 C101 269 112 222 127 192 C153 145 203 127 244 128',
    'M244 128 C244 108 244 87 244 70'
  ]);
  // The founder rejected the frontal grin. Keep the original tapered throat
  // and parallel pleats: this is whale anatomy, not a human smiling mouth.
  for(const id of ['pouch','pleat-1','pleat-2'])FRONT[id]=SRC[id].map(c=>[c[0],...c.slice(1).map((v,i)=>i%2?355+(v-355)*.65:300+(v-300)*.9)]);
  FRONT.flipper=pieces(SRC.flipper,[4,7],[
    'M165 360 C147 376 139 397 124 404',
    'M124 404 C147 408 163 402 180 390 C183 378 174 367 165 360'
  ]);
  const tailOpen=match([...SRC.flukes.slice(0,-2),['Z']],[...TARGET.flukes.slice(0,-2),['Z']]);
  FRONT.flukes=[...tailOpen.slice(0,-1),...TARGET.flukes.slice(-2)];
  // Overlap only the internal closing seam; leave the visible mark contour exact.
  for(const path of [SRC.flukes,FRONT.flukes]){path.at(-2)[1]-=2;path.at(-2)[3]-=2;}
  // Authored on a 24-cell grid, then optical-fit independently for 16/24/32.
  // These deliberately widen the throat and flukes; they are not hero downsamples.
  const SMALL_GRID={
    body:parse('M10.5 3.5 C6 3.5 3 7 3 12 C3 16.5 5 18.5 8 18.5 C9.5 18.5 10.5 18 11 17 C10.5 18.5 9 19.5 8 19.5 C9 20.5 11 20.5 12.5 20 C17 20 21 16 21.5 11.5 C22 8.5 15 10.5 12 12 C6.5 15 5.5 13 6 10 C6 8 8 6.5 10 6.5 C10.5 5.5 10.5 4.5 10.5 3.5 Z'),
    flukes:parse('M10 6.5 C11.5 6.5 11.5 8 13.5 8 C15 8 16.5 7 17 6.5 C14 6 13 5 12 6 C13 5.5 14 4.5 14.5 4 C13 3.5 12 3.5 10.5 3.5 C10.5 4.5 10.5 5.5 10 6.5 Z'),
    pouch:parse('M20.5 12.5 C20 17 16 20 11.5 19.5 C15 18.5 15.5 13 20.5 12.5 Z'),
    flipper:parse('M11 17 C10.5 18.5 9 19.5 8 19.5 C9 20.5 11 20.5 12.5 20 C13 19.5 12 18 11 17 Z'),
    'jaw-band':parse('M11 20 C17 20 21 16 22 12 C21 17 17 21 11 21 C10 21 10 20 11 20 Z'),
    eye:ellipse(14,15,0.65,.46),'eye-far':ellipse(14,15,.65,.46),eyelid:ellipse(14,15,1,.7)
  };
  const SMALL_CACHE={};
  function smallSource(size){if(SMALL_CACHE[size])return SMALL_CACHE[size];const out={};
    for(const id of Object.keys(SRC)){const path=SMALL_GRID[id];if(!path){out[id]=SRC[id];continue;}
      const fit=path.map(c=>[c[0],...c.slice(1).map(v=>{
        const cell=Math.round(v/24*size*2)/2;
        return (cell/size-.5)*634.88+256;
      })]);out[id]=match(SRC[id],orientLike(SRC[id],fit));}
    return SMALL_CACHE[size]=out;
  }
  const pathString=path=>path.map(c=>c[0]+c.slice(1).map(n=>+n.toFixed(3)).join(' ')).join(' ');
  function rotate(x,y,cx,cy,a){const c=Math.cos(a),s=Math.sin(a);return [cx+(x-cx)*c-(y-cy)*s,cy+(x-cx)*s+(y-cy)*c];}
  // Morph a blade in its shoulder-to-tip frame. A straight Cartesian blend
  // collapses a down-pointing fin when its target points up.
  function morphBlade(a,b,t){
    const aa=a[0].slice(1),bb=b[0].slice(1),at=a[4].slice(-2),bt=b[4].slice(-2);
    let angle=Math.atan2(bt[1]-bb[1],bt[0]-bb[0])-Math.atan2(at[1]-aa[1],at[0]-aa[0]);
    angle=Math.atan2(Math.sin(angle),Math.cos(angle));
    return a.map((c,i)=>{const out=[c[0]];for(let j=1;j<c.length;j+=2){
      const target=rotate(b[i][j]-bb[0],b[i][j+1]-bb[1],0,0,-angle);
      const p=rotate(lerp(c[j]-aa[0],target[0],t),lerp(c[j+1]-aa[1],target[1],t),0,0,angle*t);
      out.push(p[0]+lerp(aa[0],bb[0],t),p[1]+lerp(aa[1],bb[1],t));
    }return out;});
  }
  function build(direction,pose={},lod=0,opt={}){
    const dir=typeof direction==='string'?DIRECTIONS[direction]:direction||DIRECTIONS.mark,p={...POSE,...pose};
    // Yaw now requests a modest attentive turn. A full frontal face made the
    // mark look like a mask; the visible profile and single eye stay intact.
    const yaw=clamp(p.yaw,0,1), headYaw=smooth(0,.82,yaw)*.18,tailYaw=smooth(.24,1,yaw)*.12;
    const uncurl=dir.uncurl*(1-clamp(p.curl,0,1))+Math.max(0,-p.curl)*.9-Math.max(0,p.curl)*.08;
    const small=lod>=2, size=opt.size||24,dpr=opt.dpr||1;
    const toolWeight=clamp((p.page||0)+(p.pencil||0)+(p.lens||0)+(p.glass||0),0,1);
    const targets=[['page',392,222,132],['pencil',394,218,118],['lens',380,240,168],['glass',401,255,122]];
    let tx=335*(1-toolWeight),ty=386*(1-toolWeight),total=0;
    for(const [key,x,y,neutral] of targets){const weight=p[key]||0;total+=weight;tx+=(x+(p.fin-neutral)*.38)*weight;ty+=(y+(p.fin-neutral)*.18)*weight;}
    if(total>1){tx/=total;ty/=total;}
    const lift=smooth(30,98,p.fin),attention=smooth(.65,1,yaw)*(1-toolWeight);
    // A lifted blade has its own two continuous edges. Stretching each source
    // point by distance from the joint made the edges cross into thin spikes.
    const heldFin=pieces(SRC.flipper,[4,7],[
      `M230 398 C238 408 ${lerp(252,250,toolWeight)} ${lerp(427,425,toolWeight)} ${lerp(262,255,toolWeight)} ${lerp(438,435,toolWeight)} C${lerp(290,240,toolWeight)} ${lerp(416,350,toolWeight)} ${tx-lerp(10,35,toolWeight)} ${ty+lerp(18,5,toolWeight)} ${tx} ${ty}`,
      `M${tx} ${ty} C${tx-66} ${ty+20} 230 357 230 398`
    ]);
    const profileFin=morphBlade(small?smallSource(size).flipper:SRC.flipper,heldFin,lift);
    const posedFin=morphBlade(profileFin,FRONT.flipper,headYaw);
    const transform=(id,x,y)=>{
      const w=(1-smooth(165,295,y))*smooth(315,135,x);
      x-=uncurl*48*w;y+=uncurl*60*w;
      // Head leads, tail follows. The mark's tail roots stay attached.
      if(id==='flukes'){const hinge=lerp(220,246,tailYaw),weight=smooth(hinge,hinge+75,x);[x,y]=rotate(x,y,hinge,110,(p.fluke-6)*.48*D2R*weight);}
      if(y>285){const jawWeight=smooth(320,470,y);y+=p.mouth*22*jawWeight;
        if(id==='pouch'){y+=(p.smile-.4)*7*Math.sin((x-230)/260*Math.PI);y+=p.mouthSide*10*smooth(280,480,x)*(1-smooth(400,460,y));}}
      const headWeight=smooth(185,470,x)*smooth(150,300,y);
      [x,y]=rotate(x,y,290,305,-p.head*.34*D2R*headWeight);
      x=(x-256)/5.12;y=(y-256)/5.12;
      x*=p.scale*(1+p.squash*.5);y*=p.scale*(1-p.squash);
      [x,y]=rotate(x,y,0,0,(p.rot+p.tilt*.28)*D2R);
      if(small){x*=.9;y*=.9;}
      return [x+p.x,y+p.y];
    };
    const shapes=[];
    for(const id of ['flukes','body','jaw-band','pouch','pleat-1','pleat-2','flipper','eye','eye-far','eyelid']){
      const a=small?smallSource(size)[id]:SRC[id],b=FRONT[id],t=id==='flukes'?tailYaw:headYaw;
      let opacity=1;
      if(id==='flipper')opacity=lift*(1-attention);
      // The band already belongs to the body. An overlay caused seams while
      // turning; retain its named path for the rig without painting it twice.
      if(id==='jaw-band')opacity=0;
      if(id.startsWith('pleat')&&small)opacity=0;
      // An eye smaller than two device pixels is omitted, never made googly.
      if((id.startsWith('eye'))&&small&&26/512*size*dpr<2)opacity=0;
      if(id==='eyelid')opacity=0; // Aperture morph owns the blink; retain the named part.
      if(id==='eye-far')opacity=0; // The far side stays occluded in the modest turn.
      const path=a.map((c,i)=>{if(c[0]==='Z')return ['Z'];const out=[c[0]];for(let j=1;j<c.length;j+=2){const blend=id==='body'?lerp(headYaw,tailYaw,1-smooth(140,260,c[j+1])):t;let x=lerp(c[j],b[i][j],blend),y=lerp(c[j+1],b[i][j+1],blend);
          // Preserve the foreshortened blade through the three-quarter turn.
          // Straight interpolation accidentally aligned these anchors vertically.
          if(id==='body')x-=8*Math.sin(yaw*Math.PI)*smooth(90,0,Math.hypot(c[j]-160,c[j+1]-442));
          if(id==='flipper'){x=posedFin[i][j];y=posedFin[i][j+1];}
          if(yaw>0)x=256+(x-256)*lerp(1,dir.frontWidth,headYaw);
          if(id==='eye'||id==='eye-far'){
            const cx=id==='eye'?lerp(294,200,headYaw):lerp(294,365,headYaw),cy=lerp(337,330,headYaw);
            const visible=id==='eye-far'?0:1;
            x=cx+(x-cx)*p.eyeScale*visible;y=cy+(y-cy)*p.eyeScale*(1-p.lid*.97-p.lidLow*.64)*visible;
            y+=p.lidLow*3*(Math.pow((x-cx)/13,2)-1);
            x+=p.lookX*2;y+=p.lookY*2;
          }
          const xy=transform(id,x,y),grid=124/(size*dpr);
          out.push(...(small?xy.map(v=>Math.round(v/grid*2)/2*grid):xy));}return out;});
      shapes.push({id,path,role:['pouch','eye','eye-far'].includes(id)?'hole':'body',opacity});
    }
    // Keep the legacy named lid geometry; closing the eye narrows its aperture.
    const lid=shapes.find(s=>s.id==='eyelid');lid.opacity=0;
    lid.path=SRC.eyelid.map((c,i)=>{if(c[0]==='Z')return ['Z'];let o=[c[0]];for(let j=1;j<c.length;j+=2){const x=lerp(c[j],FRONT.eyelid[i][j],headYaw),cy=lerp(337,328,headYaw),y=lerp(c[j+1],FRONT.eyelid[i][j+1],headYaw);o.push(...transform('eye',x,cy-13+(y-cy+15)*p.lid*.40));}return o;});
    const anchor=(x,y)=>{const z=transform('body',x,y);return {x:z[0],y:z[1]};};
    const tip=shapes.find(s=>s.id==='flipper').path[4].slice(-2);
    // A tapered negative-space wedge is the mark's existing flipper separation,
    // carried up with the arm. It is a filled shape, not an outline or shadow.
    const left=[[230,398],[230,357],[tx-66,ty+20],[tx,ty]],mix=(a,b,t)=>a.map((v,i)=>lerp(v,b[i],t));
    const q1=mix(left[0],left[1],.42),mid=mix(left[1],left[2],.42),q2=mix(q1,mid,.42),q3=mix(q2,mix(mid,mix(left[2],left[3],.42),.42),.42);
    const point=(p,dx=0,dy=0)=>transform('body',p[0]+dx,p[1]+dy);
    const gap={id:'flipper-gap',path:[['M',...point(left[0],-6,-4)],['C',...point(q1,-7,-2),...point(q2,-5,-1),...point(q3)],['C',...point(q2),...point(q1),...point(left[0],-6,-4)],['Z']],role:'hole',opacity:lift*(1-headYaw)*(1-attention)};
    shapes.splice(shapes.findIndex(s=>s.id==='flipper'),0,gap);
    const tail=transform('flukes',lerp(289,296,tailYaw),lerp(113,121,tailYaw));
    const anchors={blowhole:anchor(lerp(325,278,headYaw),lerp(234,278,headYaw)),spout:anchor(410,222),mouth:anchor(lerp(480,426,headYaw),lerp(300,353,headYaw)),chin:anchor(lerp(397,285,headYaw),lerp(376,425,headYaw)),eye:anchor(lerp(294,200,headYaw),lerp(337,330,headYaw)),
      finTip:{x:tip[0],y:tip[1]},tail:{x:tail[0],y:tail[1]},tailAng:(p.rot+p.tilt*.28+(p.fluke-6)*.35)*D2R,
      finAng:(p.rot+p.tilt*.28)*D2R,headAng:(p.rot+p.tilt*.28-p.head*.34)*D2R,eyeR:2.5};
    return {shapes,anchors,pose:p,lod,small,size,dpr,unitScale:p.scale*(small?.9:1)};
  }
  function resolveLook(theme='paper',px=1,dpr=1,lod=0){return {theme,px,dpr,lod,body:theme==='charcoal'?['#3594D8','#2B70D5']:['#1E8FD8','#0B48BB'],hole:'#faf8f5',ink:theme==='mono'?'#000000':theme==='charcoal'?'#faf8f5':'#202123',mono:theme==='mono',bg:theme==='charcoal'?'#202123':'#faf8f5'};}
  function trace(ctx,path){for(const c of path){if(c[0]==='M')ctx.moveTo(c[1],c[2]);else if(c[0]==='C')ctx.bezierCurveTo(...c.slice(1));else ctx.closePath();}}
  function holesFor(id,shapes,mono=true){const prefix=id.slice(0,-4);
    return shapes.filter(h=>(mono&&id.endsWith('body')&&h.role==='hole'&&[prefix+'pouch',prefix+'eye',prefix+'eye-far'].includes(h.id))||
      (mono&&h.id==='flipper-gap'&&id==='body')||
      (h.role==='cutout'&&['prop-page','prop-pad'].includes(id)&&h.id.startsWith(id+'-'))).map(h=>{
      const origin=h.path[0].slice(1),w=h.opacity;
      return h.path.map(c=>[c[0],...c.slice(1).map((v,i)=>lerp(origin[i%2],v,w))]);
    });}
  function draw(ctx,parts,look){const g=ctx.createLinearGradient(-50,-50,50,50);g.addColorStop(0,look.body[0]);g.addColorStop(1,look.body[1]);
    for(const s of parts.shapes){if(s.opacity<=0||s.role==='cutout'||(look.mono&&s.role==='hole'))continue;ctx.save();ctx.globalAlpha*=s.opacity;ctx.beginPath();trace(ctx,s.path);
      for(const hole of holesFor(s.id,parts.shapes,look.mono))trace(ctx,hole);
      ctx.fillStyle=look.mono?look.ink:s.role==='hole'?look.hole:s.role==='pointer'?(look.theme==='charcoal'?'#66d6de':'#147888'):s.role==='accent'?'#D2A34E':s.role==='paper'?look.hole:s.role==='water'?look.body[0]:s.role==='tool'?look.ink:g;
      ctx.fill('evenodd');ctx.restore();}return look;}
  function svg(parts,theme='paper',size=512){const look=resolveLook(theme);
    const paths=parts.shapes.map(s=>{let opacity=s.opacity,d=pathString(s.path);d+=' '+holesFor(s.id,parts.shapes,look.mono).map(pathString).join(' ');if(s.role==='cutout'||(look.mono&&s.role==='hole'))opacity=0;
      const fill=look.mono?'currentColor':s.role==='hole'||s.role==='paper'?look.hole:s.role==='pointer'?(theme==='charcoal'?'#66d6de':'#147888'):s.role==='accent'?'#D2A34E':s.role==='water'?look.body[0]:s.role==='tool'?look.ink:'url(#whale-blue)';
      return `<path id="${s.id}" d="${d}" fill="${fill}" fill-rule="evenodd" opacity="${+opacity.toFixed(4)}"/>`;}).join('\n');
    return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="-62 -62 124 124" width="${size}" height="${size}" role="img" aria-label="Codewhale character"><defs><linearGradient id="whale-blue" gradientUnits="userSpaceOnUse" x1="-50" y1="-50" x2="50" y2="50"><stop stop-color="${look.body[0]}"/><stop offset="1" stop-color="${look.body[1]}"/></linearGradient></defs>\n${paths}\n</svg>`;
  }
  root.WhaleRig={TAU,D2R,lerp,clamp,smooth,POSE,DIRECTIONS,build,resolveLook,draw,svg,pathString,parse,ellipse,trace,signedArea,holesFor,sourceParts:SRC};
})(typeof window!=='undefined'?window:globalThis);
