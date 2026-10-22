(()=>{"use strict";
const w=globalThis.webkit,h=w&&w.messageHandlers&&w.messageHandlers.zephiumPageAssistBootV1;
if(!h)return;
const post=h.postMessage,apply=Reflect.apply,run=Function;
let serve=null,pulling=false;
const pull=()=>{if(pulling)return;pulling=true;
apply(post,h,[serve===null?"B1":"B2"]).then(a=>{pulling=false;
if(typeof a!=="string"||a==="S1")return;
if(serve===null){try{serve=run(a)();}catch(_){serve=null;}
if(typeof serve!=="function"){serve=null;apply(post,h,["E1"]).catch(()=>{});return;}}
else if(a!=="G1")return;
Promise.resolve().then(serve).then(pull,pull);},()=>{pulling=false;});};
addEventListener("pageshow",e=>{if(e.persisted)pull();},true);
pull();})();
