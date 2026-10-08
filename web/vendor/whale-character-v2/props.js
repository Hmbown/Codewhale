// Flat physical props; shared paths for the canvas and standalone SVG exports.
(function(root){
  'use strict';
  const R=root.WhaleRig,{clamp,D2R}=R;
  const poly=pts=>{const a=[['M',...pts[0]]];for(let i=1;i<=pts.length;i++){const p=pts[(i-1)%pts.length],q=pts[i%pts.length];a.push(['C',p[0],p[1],q[0],q[1],q[0],q[1]]);}a.push(['Z']);return a;};
  function transform(path,x,y,a=0,s=1){const c=Math.cos(a),sn=Math.sin(a);return path.map(cmd=>{const o=[cmd[0]];for(let i=1;i<cmd.length;i+=2)o.push(x+(cmd[i]*c-cmd[i+1]*sn)*s,y+(cmd[i]*sn+cmd[i+1]*c)*s);return o;});}
  function shapes(p,parts,view){const small=(view.lod||0)>=2,out=[],boost=(small?1.15:1)*parts.unitScale;
    const add=(id,path,role,x,y,a,s,visible)=>out.push({id,path:transform(path,x,y,a,s),role,opacity:clamp(visible||0,0,1)});
    const hand=parts.anchors.finTip,angle=parts.anchors.finAng;
    const offset=(anchor,x,y,a=angle)=>({x:anchor.x+(x*Math.cos(a)-y*Math.sin(a))*boost,y:anchor.y+(x*Math.sin(a)+y*Math.cos(a))*boost});
    const page=R.parse('M-9 -13 C-5 -14 0 -14 4 -13 C6 -11 7 -9 9 -8 C9 -1 9 6 8 13 C3 12 -3 13 -9 14 C-8 5 -8 -4 -9 -13 Z'),fold=poly([[4,-13],[4,-8],[9,-8]]);
    // Optical separation: a literal air gap survives at 24 px and in one ink.
    const pc=offset(hand,22+(p.pageDX||0),-31+(p.pageDY||0)),px=pc.x,py=pc.y,pa=angle-.08+(p.pageRot||0)*D2R;
    add('prop-page',page,'tool',px,py,pa,boost,p.page);
    add('prop-page-fold',fold,'cutout',px,py,pa,boost,p.page);
    for(let i=0;i<2;i++)add(`prop-page-rule-${i}`,poly([[-5,-4+i*6],[5,-4+i*6],[5,-2+i*6],[-5,-2+i*6]]),'cutout',px,py,pa,boost,p.page);
    // The page turns out and returns, rather than resetting a 1 to 0 in one frame.
    add('prop-page-turn',page,'tool',px+2*boost,py,pa,boost*Math.sin((p.pageFlip||0)*Math.PI/2)*.75,p.page*(p.pageFlip||0));
    const pad=offset(hand,21,-16);
    add('prop-pad',poly([[-11,-5],[11,-5],[11,5],[-11,5]]),'tool',pad.x,pad.y,angle-.05,boost,p.pad);
    add('prop-pad-binding',poly([[-8.5,-3],[8.5,-3],[8.5,-1.5],[-8.5,-1.5]]),'cutout',pad.x,pad.y,angle-.05,boost,p.pad);
    for(let i=0;i<4;i++)add(`prop-pad-line-${i}`,poly([[-8+i*4,0],[-6+i*4,0],[-6+i*4,1.2],[-8+i*4,1.2]]),'cutout',pad.x,pad.y,angle-.05,boost,p.pad*clamp((p.padLines||0)-i,0,1));
    const pencil=poly([[-2,-5],[2,-5],[2,6],[0,10],[-2,6]]);
    const pen=offset(hand,16.4+(p.scribX||0)*.35,-23.3+(p.scribY||0)*.35);
    add('prop-pencil',pencil,'accent',pen.x,pen.y,angle-.48,boost,p.pencil);
    add('prop-pencil-tip',poly([[-1.3,6.5],[1.3,6.5],[0,10]]),'tool',pen.x,pen.y,angle-.48,boost,p.pencil);
    const lens=[...R.ellipse(0,0,9,9),...R.ellipse(0,0,5.8,5.8)];
    const la=angle+.55,lc=offset(hand,3,-42,la);
    add('prop-lens',lens,'tool',lc.x,lc.y,la,boost,p.lens);
    add('prop-lens-handle',poly([[-1.7,7],[1.7,7],[1.7,17],[-1.7,17]]),'tool',lc.x,lc.y,la,boost,p.lens);
    const wrench=poly([[-2,9],[-2,-8],[-6,-11],[-6,-18],[-3,-21],[-3,-14],[3,-14],[3,-21],[6,-18],[6,-11],[2,-8],[2,9],[5,12],[5,17],[2,20],[2,14],[-2,14],[-2,20],[-5,17],[-5,12]]);
    const wc=offset(parts.anchors.tail,40,-5+(p.wrenchY||0));
    add('prop-wrench',wrench,'tool',wc.x,wc.y,parts.anchors.tailAng+1.1+(p.wrenchSpin||0)*D2R,(small?.78:.7)*parts.unitScale,p.wrench);
    const ga=angle-.22,gs=boost*(.95+.3*clamp(p.glassExt||0,0,1)),gc=offset(hand,12,-33,ga);
    add('prop-spyglass',poly([[0,-2.5],[6,-2.5],[6,-3.5],[13,-3.5],[13,-4.5],[19,-4.5],[19,4.5],[13,4.5],[13,3.5],[6,3.5],[6,2.5],[0,2.5]]),'accent',gc.x,gc.y,ga,gs,p.glass);
    add('prop-spyglass-end',poly([[17,-4.5],[20,-4.5],[20,4.5],[17,4.5]]),'tool',gc.x,gc.y,ga,gs,p.glass);
    // Pointer proportions follow the existing Codewhale Computer Use mark.
    const cursor=offset(parts.anchors.spout,11+(p.cursorX||0),-31+(p.cursorY||0));
    add('prop-cursor',poly([[-8,-12],[10,1.6],[1.6,2.8],[-2.2,10.4]]),'pointer',cursor.x,cursor.y,angle,boost,p.cursor);
    const link=offset(parts.anchors.spout,16,-34),linkAngle=angle+(.55+(p.linkTilt||0)*D2R);
    for(const side of [-1,1])add(`prop-link-${side}`,[...R.ellipse(side*2.3,side*5.5,4.5,8),...R.ellipse(side*2.3,side*5.5,2.1,5.3)],'pointer',link.x,link.y,linkAngle,boost,p.link);
    // A recognisable fan-shaped jet, with a separate water surface below it.
    // These are filled silhouettes, so the fountain survives one-ink rendering.
    const jet=R.parse('M-2 0 C-3 -12 -9 -28 -17 -29 C-22 -29 -23 -24 -19 -21 C-25 -21 -27 -28 -23 -32 C-16 -39 -5 -32 0 -16 C5 -33 16 -38 23 -31 C27 -26 23 -20 19 -21 C23 -25 20 -29 16 -28 C8 -26 4 -12 2 0 C1 1 -1 1 -2 0 Z');
    const spout=clamp(p.spout||0,0,1),surface=clamp(p.splash||0,0,1),origin=parts.anchors.spout;
    add('prop-spout',jet,'water',origin.x,origin.y,0,(.35+.65*spout)*parts.unitScale,spout);
    for(const side of [-1,1])add(`prop-spout-drop-${side}`,R.parse('M0 -3 C3 0 3 3 0 3 C-3 3 -3 0 0 -3 Z'),'water',origin.x+side*25*parts.unitScale,origin.y-35*parts.unitScale,-side*.35,parts.unitScale*spout,spout);
    add('prop-water-back',R.parse('M-43 0 C-28 -7 -12 -4 0 -1 C14 2 28 -7 43 -2 C29 -1 15 7 0 3 C-15 -1 -28 -3 -43 0 Z'),'water',0,43,0,.8+.2*surface,surface);
    add('prop-water-front',R.parse('M-29 0 C-11 -2 8 4 28 0 C13 8 -11 4 -29 0 Z'),'water',0,50,0,.8+.2*surface,surface);
    const splash=R.parse('M0 0 C-2 -8 -8 -12 -11 -15 C-5 -15 2 -8 3 0 C2 2 1 2 0 0 Z');
    for(const side of [-1,1])add(`prop-water-splash-${side}`,splash.map(c=>[c[0],...c.slice(1).map((v,i)=>i%2?v:-side*v)]),'water',side*35,39,0,.5+.5*surface,surface);
    return out;
  }
  function particles(list,view){const out=[],small=(view.lod||0)>=2;
    for(const [i,p] of list.entries()){
      if(p.age<0||p.kind==='mist'||(small&&p.kind==='bubble'))continue;
      const fade=clamp((p.life-p.age)/(p.life*.2),0,1),r=p.r*(small?1.65:1);
      let x=p.x,y=p.y;
      if(small&&p.kind==='thought'){x+=4;y-=6;}
      if(p.kind==='drop'){
        const shape=R.parse('M0 -2 C1 -1 2 1 0 2 C-2 1 -1 -1 0 -2 Z');
        out.push({id:`spout-${i}`,path:transform(shape,x,y,0,r),role:'water',opacity:fade});
      }else{
        const path=p.cloud?R.parse('M-5 1 C-8 0 -7 -4 -4 -4 C-3 -8 2 -8 4 -5 C8 -5 9 0 5 2 C4 6 -3 6 -5 1 Z'):R.ellipse(0,0,1,1);
        out.push({id:`bubble-${i}`,path:transform(path,x,y,0,p.cloud?r/6:r),role:'water',opacity:fade});
      }
    }
    return out;
  }
  root.WhaleProps={shapes,particles};
})(typeof window!=='undefined'?window:globalThis);
