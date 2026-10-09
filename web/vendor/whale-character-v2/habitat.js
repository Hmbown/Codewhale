// The companion's little cove. It uses the existing animation clock and stops
// with reduced motion / page visibility; it never changes Engine presence.
(function(root){
  'use strict';
  const R=root.WhaleRig,TAU=Math.PI*2;
  let target={x:0,y:0},gaze={x:0,y:0},lastFrame=0;
  const ripples=[];
  function observe(x,y){target={x:R.clamp(x/62,-1,1),y:R.clamp(y/62,-1,1)};}
  function leave(){target={x:0,y:0};}
  function tap(x,y,d){
    const point={x:R.clamp(x,-42,42),y:R.clamp(y,25,48),at:d.f};
    ripples.push(point);if(ripples.length>3)ripples.shift();observe(point.x,point.y);
  }
  function pose(d){
    const dt=Math.max(0,Math.min(.25,(d.f-lastFrame)/30));lastFrame=d.f;
    if(d.reduced){gaze={x:0,y:0};return {};}
    const active=d.acting==='rest'&&!d.reduced,w=1-Math.exp(-dt*4);
    gaze.x=R.lerp(gaze.x,active?target.x:0,w);gaze.y=R.lerp(gaze.y,active?target.y:0,w);
    const p=d.pose();return {lookX:p.lookX+gaze.x*.85,lookY:p.lookY+gaze.y*.65,head:p.head-gaze.y*5,rot:p.rot+gaze.x*2};
  }
  function palette(theme){return theme==='charcoal'
    ? {air:'#243c4b',shore:'#426878',water:'#254e64',deep:'#1d4055',reed:'#4c8490',glint:'#699cab'}
    : {air:'#edf5f5',shore:'#c4d8d7',water:'#d6eaed',deep:'#b9dce3',reed:'#86b5b6',glint:'#f7fcfb'};}
  function fill(ctx,d,color,alpha=1){ctx.save();ctx.globalAlpha=alpha;ctx.fillStyle=color;ctx.beginPath();R.trace(ctx,R.parse(d));ctx.fill();ctx.restore();}
  function behind(ctx,d,theme){
    const p=palette(theme),t=d.reduced?0:d.f/30,w=Math.sin(t*.55)*1.4;
    fill(ctx,'M-59 7 C-63 -29 -37 -58 0 -59 C35 -62 60 -41 61 -6 C66 31 48 57 13 60 C-22 67 -54 48 -59 7 Z',p.air);
    fill(ctx,'M-61 29 C-61 13 -56 5 -51 11 C-46 16 -48 22 -43 25 C-39 27 -37 30 -35 34 C-45 37 -55 36 -61 29 Z',p.shore);
    fill(ctx,`M-60 ${21+w} C-37 ${15+w} -14 ${23-w} 6 21 C26 ${18+w} 43 ${15-w} 61 18 C61 42 42 59 12 61 C-19 65 -51 47 -60 ${21+w} Z`,p.water);
    fill(ctx,'M-55 40 C-29 49 -9 42 12 45 C32 49 44 39 56 36 C44 56 21 63 -3 61 C-25 59 -44 49 -55 40 Z',p.deep,.7);
    for(const [x,h,phase] of [[-54,29,0],[-49,22,1],[53,19,2],[57,26,3]]){
      const sway=Math.sin(t*.65+phase)*2;
      fill(ctx,`M${x} 52 C${x-2} 42 ${x+sway-5} ${52-h+5} ${x+sway} ${52-h} C${x+sway-1} ${52-h+9} ${x+3} 40 ${x+1} 52 Z`,p.reed,.8);
    }
    fill(ctx,`M-53 33 C-45 ${30+w} -40 ${30+w} -34 33 C-42 32 -47 34 -53 33 Z`,p.glint,.7);
    fill(ctx,`M35 30 C43 ${28-w} 48 ${28-w} 54 30 C47 30 42 32 35 30 Z`,p.glint,.65);
    // Water bubbles live low in the cove, away from the thinking-cloud region.
    for(let i=0;i<3;i++){
      const phase=d.reduced?.35:(t/(7+i*2)+i*.31)%1;
      const x=[-47,47,39][i]+Math.sin(t*.6+i)*1.2,y=55-phase*24,r=.55+phase*.5;
      ctx.save();ctx.globalAlpha=Math.sin(phase*Math.PI)*.65;ctx.fillStyle=p.glint;ctx.beginPath();R.trace(ctx,R.ellipse(x,y,r,r));ctx.fill();ctx.restore();
    }
  }
  function front(ctx,d,theme){
    const p=palette(theme),t=d.reduced?0:d.f/30,w=Math.sin(t*.45)*2;
    // The near current passes below the face, preserving the white crescent.
    fill(ctx,`M-40 51 C-23 ${47+w} -9 ${54-w} 8 51 C21 ${48+w} 30 49 40 47 C26 54 16 54 5 54 C-11 57 -26 50 -40 51 Z`,p.glint,.6);
    fill(ctx,`M-23 58 C-8 ${56-w*.3} 8 60 21 57 C9 62 -8 60 -23 58 Z`,p.glint,.55);
    for(const ripple of ripples){
      const age=d.reduced?12:d.f-ripple.at;if(age<0||age>78)continue;
      for(let i=0;i<2;i++){
        const a=age-i*9;if(a<0)continue;const radius=2+a*.19,alpha=(1-a/78)*.72;
        ctx.save();ctx.globalAlpha=alpha;ctx.fillStyle=p.glint;ctx.beginPath();
        R.trace(ctx,R.ellipse(ripple.x,ripple.y,radius,radius*.25));
        R.trace(ctx,R.ellipse(ripple.x,ripple.y,Math.max(0,radius-.7),Math.max(0,radius*.25-.55)));
        ctx.fill('evenodd');ctx.restore();
      }
    }
  }
  root.WhaleHabitat={behind,front,pose,observe,leave,tap,inspect:()=>({target:{...target},gaze:{...gaze},ripples:ripples.length})};
})(window);
