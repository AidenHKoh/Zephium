(() => {
  "use strict";
  const NAME="__zephiumRecordMediaV1";
  if(Object.hasOwn(globalThis,NAME))return;
  const snapshots=new Map(), style=getComputedStyle, rect=Element.prototype.getBoundingClientRect;
  const fetchAsset=fetch, encode=btoa, stringify=JSON.stringify, parse=JSON.parse;
  const MAX_RAW=524288, MAX_OUTPUT=65536;
  let pending=null;
  addEventListener("pagehide",()=>{if(pending)pending.abort();snapshots.clear();},true);
  function source(node,kind) {
    if(kind==="image")return node.currentSrc||"";
    if(kind==="poster")return node.poster||"";
    if(kind==="page_image")return node.content||"";
    if(kind==="background") {
      const value=style(node).backgroundImage;
      const match=/^url\(["']?([^"'\\]+?)["']?\)$/.exec(value);
      return match && match[1].length<=4096?match[1]:"";
    }
    return "";
  }
  function observedImageSources(node) {
    const out=new Set();let work=0;
    const add=value=>{if(!value || value.length>4096 || ++work>32)return;try{out.add(new URL(value,document.baseURI).href);}catch(_){}};
    const sources=element=>{
      for(const attr of ["src","data-src"])add(element.getAttribute(attr));
      for(const attr of ["srcset","data-srcset"]) {
        const value=element.getAttribute(attr)||"";if(value.length>8192)continue;
        for(const item of value.split(",").slice(0,16)) {
          const match=/^\s*(\S+)(?:\s+(?:[0-9]+w|[0-9.]+x))?\s*$/.exec(item);
          if(match && !match[1].startsWith("data:"))add(match[1]);
        }
      }
    };
    sources(node);
    if(node.parentElement?.localName==="picture")for(const sibling of [...node.parentElement.children].slice(0,5))if(sibling.localName==="source")sources(sibling);
    return out;
  }
  function label(node) {
    const stack=[node];let text="",visited=0;
    while(stack.length) {
      const current=stack.pop();if(++visited>64)return null;
      if(current.nodeType===3){const value=current.nodeValue;if(value.length+text.length>2048)return null;text+=value;}
      else if(current.nodeType===1) {for(const child of current.childNodes){if(stack.length>=64)return null;stack.push(child);}}
    }
    const aria=node.getAttribute("aria-label")||"";return aria.length<=2048?text+"|"+aria:null;
  }
  function sharedCard(a,b) {
    let parent=b;
    for(let depth=0;depth<8 && parent && parent.nodeType===1;depth++,parent=parent.parentNode) {
      if(["html","body","main","nav","header","footer","aside"].includes(parent.localName))return null;
      if(parent.contains(a))return parent;
    }
    return null;
  }
  function remember(i,g,records,rows) {
    const entries=new Map();
    for(const row of rows) {
      const record=records.find(record=>record.wire.k===row.k);if(!record)continue;
      const names=new Map();
      for(const key of row.r) {
        const name=records.find(record=>record.wire.k===key);if(!name)continue;
        const card=sharedCard(record.element,name.element);
        const text=label(name.element);
        if(text!==null && (card || row.t==="page_image")) names.set(key,{node:new WeakRef(name.element),card:card?new WeakRef(card):null,text});
      }
      const url=source(record.element,row.t);if(url.length>4096)continue;
      const content=row.t==="record"?label(record.element):null;
      if(row.t==="record" && content===null)continue;
      entries.set(row.k,{node:new WeakRef(record.element),kind:row.t,content,source:url,lazy:row.t==="image" && !url?observedImageSources(record.element):null,names});
    }
    const key=i+":"+g;snapshots.delete(key);snapshots.set(key,entries);
    while(snapshots.size>8)snapshots.delete(snapshots.keys().next().value);
  }
  function evidence(records, invocation, generation, helpers) {
    const {apply,read,nodeContains,elementRect,tagName,nodeType,nodeParentGetter,imageCompleteGetter,imageNaturalWidthGetter,pageTitleText}=helpers;
    const candidates=records.filter(record=>record.media!==null && record.sensitivity!=="secret" &&
      (record.media==="page_image" || record.wire.b && record.wire.b.w>=64 && record.wire.b.h>=48));
    let work=0;
    const linkProofs=new WeakMap(),provenNames=new Set();
    const links=new WeakSet(records.filter(r=>r.wire.r==="link").map(r=>r.element));
    const counts=new WeakMap(),textOwners=new WeakSet(),imageOwners=new WeakMap(),boundaries=new WeakMap();
    const indexAncestors=(items,visit)=>{
      for(let index=0;index<items.length;index++) {
        let node=items[index].element,depth=0;
        while(node && nodeType(node)===1 && !["html","body"].includes(tagName(node))) {
          if(++work>16384 || ++depth>64)return false;
          visit(node,index);node=read(nodeParentGetter,node);
        }
      }
      return true;
    };
    const titles=records.filter(record=>record.wire.r==="heading");
    const identities=records.filter(record=>
      ["heading","paragraph"].includes(record.wire.r) && (record.wire.n || record.wire.t) ||
      record.wire.r==="link" && label(record.element)?.split("|")[0].trim());
    if(!indexAncestors(titles,node=>counts.set(node,Math.min(2,(counts.get(node)||0)+1))) ||
       !indexAncestors(identities,node=>textOwners.add(node)))return [];
    // Same64-node/stack bound as labels; no text or field values are read.
    const uniqueImage=(root,wanted)=>{
      const stack=[root];let visited=0,found=false;
      while(stack.length) {
        const node=stack.pop();if(++visited>64 || ++work>16384)return false;
        if(nodeType(node)!==1)continue;
        const tag=tagName(node);
        if(node.shadowRoot || tag.includes("-") || ["iframe","frame","object","embed"].includes(tag))return false;
        if(node===wanted)found=true;
        else if(["img","video","canvas","svg"].includes(tag) || style(node).backgroundImage!=="none")return false;
        for(const part of ["::before","::after"]) {
          const css=style(node,part);if(css.backgroundImage!=="none" || css.content.includes("url("))return false;
        }
        for(const child of node.childNodes){if(stack.length>=64)return false;stack.push(child);}
      }
      return found;
    };
    const cardBoundary=element=>{
      const tag=tagName(element),role=element.getAttribute("role");
      if(tag==="article" || tag==="li" || role==="listitem")return true;
      if(!["div","section","figure"].includes(tag) && !tag.includes("-") && role!=="group")return false;
      if(boundaries.has(element))return boundaries.get(element);
      const parent=element.parentElement;let boundary=false;
      if(parent && textOwners.has(element)) {
        if(parent.children.length>64)boundary=true;
        else for(const peer of parent.children) {
          if(++work>16384)return true;
          if(peer!==element && peer.localName===tag && textOwners.has(peer)){boundary=true;break;}
        }
      }
      boundaries.set(element,boundary);return boundary;
    };
    const images=candidates.filter(record=>record.media!=="record").slice(0,48);
    for(const record of candidates) {
      if(record.media!=="record" || images.length>=48)continue;
      const node=record.element,b=record.wire.b;
      if(b.w*b.h>innerWidth*innerHeight/2 || label(node)===null || node.children.length>64 ||
          node.querySelector("input,textarea,select,iframe,frame,object,embed,[contenteditable]"))continue;
      let nested=false;
      for(const other of candidates) {
        if(++work>16384)return [];
        if(other!==record && other.media==="record" && apply(nodeContains,node,[other.element])){nested=true;break;}
      }
      if(nested || (counts.get(node)||0)>1 || [...node.children].some(child=>cardBoundary(child)))continue;
      images.push(record);
    }
    if(work>16384 || !images.length)return [];
    if(!indexAncestors(images,(node,index)=>{
      const entries=imageOwners.get(node)||[];entries.push(index);imageOwners.set(node,entries);
    }))return [];
    const rows=images.map(record=>({k:record.wire.k,t:record.media,r:[],l:record.media!=="image" ||
      read(imageCompleteGetter,record.element)===true && read(imageNaturalWidthGetter,record.element)>=32}));
    const inside=(outer,inner)=>outer && inner && inner.x>=outer.x-2 && inner.y>=outer.y-2 &&
      inner.x+inner.w<=outer.x+outer.width+2 && inner.y+inner.h<=outer.y+outer.height+2;
    association: for(const record of records) {
      if(!["heading","link","paragraph","document","list_item"].includes(record.wire.r) ||
         !(record.wire.n || record.wire.t) || record.sensitivity==="secret")continue;
      let parent=record.element;
      for(let depth=0;depth<8 && parent && parent!==document;depth++) {
        if(++work>16384)break association;
        if(nodeType(parent)!==1 || ["html","body","main","nav","header","footer","aside"].includes(tagName(parent)))break;
        if((counts.get(parent)||0)>1)break;
        const box=elementRect(parent),related=[];let visual=-1,visuals=0;
        for(const index of imageOwners.get(parent)||[]) {
          if(++work>16384)break association;
          const image=images[index];
          if(image.media!=="page_image" && inside(box,image.wire.b) && inside(box,record.wire.b)) {
            related.push(index);if(image.media!=="record"){visual=index;visuals++;}
          }
        }
        if(related.length) {
          for(const index of related)if(rows[index].r.length<8)rows[index].r.push(record.wire.k);
          if(visuals)provenNames.add(record.wire.k);
          const owned=imageOwners.get(record.element);
          if(visuals===1 && record.wire.u && owned?.length===1 && owned[0]===visual && links.has(record.element) && rows[visual].r.includes(record.wire.k) && uniqueImage(record.element,images[visual].element))
            linkProofs.set(record.element,{index:visual,parent});
          break;
        }
        if(cardBoundary(parent))break;
        if(work>16384)break association;
        parent=read(nodeParentGetter,parent);
      }
      if(pageTitleText(record.element,tagName(record.element))!==null) {
        for(let index=0;index<images.length;index++) {
          if(++work>16384)break association;
          if(images[index].media==="page_image" && rows[index].r.length<4)rows[index].r.push(record.wire.k);
        }
      }
    }
    // A captured paragraph may inherit only its own enclosing link's already
    // proven single-image association. No labels, URL equality, or new card rule.
    inheritance: for(const record of records) {
      if(record.wire.r!=="paragraph" || record.sensitivity==="secret" || provenNames.has(record.wire.k))continue;
      let parent=read(nodeParentGetter,record.element);
      for(let depth=0;depth<8 && parent;depth++,parent=read(nodeParentGetter,parent)) {
        if(++work>16384)break inheritance;
        if(nodeType(parent)!==1 || ["html","body","main","nav","header","footer","aside","article","li"].includes(tagName(parent)) || parent.getAttribute("role")==="listitem" || (counts.get(parent)||0)>1)break;
        if(links.has(parent)) {
          const proof=linkProofs.get(parent);
          if(proof) {
            const row=rows[proof.index],box=elementRect(proof.parent);
            if(row.r.length<8 && inside(box,record.wire.b) && inside(box,images[proof.index].wire.b))row.r.push(record.wire.k);
          }
          break;
        }
        if(cardBoundary(parent)) {
          const siblings=parent.parentElement;if(work>16384 || !siblings || siblings.children.length>64)break;
          let separate=false;
          for(const peer of siblings.children) {
            if(++work>16384)break inheritance;
            if(peer!==parent && imageOwners.has(peer)){separate=true;break;}
          }
          if(separate)break;
        }
      }
    }
    remember(invocation,generation,records,rows);
    return rows;
  }

  function target(request) {
    const entry=snapshots.get(request.i+":"+request.g)?.get(request.k),name=entry?.names.get(request.n);
    const node=entry?.node.deref(),named=name?.node.deref(),card=name?.card?.deref();
    if(!node?.isConnected || !named?.isConnected || label(named)!==name.text ||
       (entry.kind!=="page_image" && (!card?.isConnected || !card.contains(node) || !card.contains(named))))return null;
    if(entry.kind==="record" && label(node)!==entry.content)return null;
    const url=source(node,entry.kind);
    if(url!==entry.source && !(entry.kind==="image" && entry.lazy?.has(url) && node.complete && node.naturalWidth>=32))return null;
    return {entry,node,url};
  }
  function crop(request,privacy) {
    const current=target(request);if(!current || current.entry.kind==="page_image")return {v:1,s:"unavailable"};
    const box=rect.call(current.node),computed=style(current.node);
    if(computed.visibility!=="visible" || computed.display==="none" || box.width<64 || box.height<48 ||
       box.x<0 || box.y<0 || box.right>innerWidth || box.bottom>innerHeight)return {v:1,s:"unavailable"};
    if(current.entry.kind==="image" && (!current.node.complete || current.node.naturalWidth<32))return {v:1,s:"unavailable"};
    let proof;try{proof=parse(privacy());}catch(_){return {v:1,s:"unavailable"};}if(!proof.c)return {v:1,s:"unavailable"};
    return {v:1,s:"crop",p:proof,i:request.i,g:request.g,k:request.k,n:request.n,b:{x:box.x,y:box.y,w:box.width,h:box.height}};
  }
  function dimensions(bytes,type) {
    const view=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength),n=bytes.length;
    const u16=(at)=>at+2<=n?view.getUint16(at):0,u32=(at)=>at+4<=n?view.getUint32(at):0;
    const le24=(at)=>at+3<=n?bytes[at]|bytes[at+1]<<8|bytes[at+2]<<16:0;
    const word=(at,value)=>at+value.length<=n && [...value].every((c,i)=>bytes[at+i]===c.charCodeAt(0));
    let size=null;
    if(type==="image/png" && n>=33 && u32(0)===0x89504e47 && u32(4)===0x0d0a1a0a && word(12,"IHDR") && u32(8)===13) {
      for(let at=8;at+12<=n;){const length=u32(at);if(length>n-at-12 || word(at+4,"acTL"))return null;at+=length+12;}
      size=[u32(16),u32(20)];
    } else if(type==="image/jpeg" && n>=4 && u16(0)===0xffd8) {
      for(let at=2;at+4<=n;) {
        if(bytes[at++]!==255)return null;while(at<n && bytes[at]===255)at++;
        const marker=bytes[at++];if(marker===0xda || marker===0xd9)break;
        if(marker===0x01 || marker>=0xd0 && marker<=0xd7)continue;
        const length=u16(at);if(length<2 || at+length>n)return null;
        if([0xc0,0xc1,0xc2].includes(marker)) {if(length<8)return null;size=[u16(at+5),u16(at+3)];break;}
        at+=length;
      }
    } else if(type==="image/webp" && n>=30 && word(0,"RIFF") && word(8,"WEBP")) {
      if(word(12,"VP8X")) {if(bytes[20]&2)return null;size=[le24(24)+1,le24(27)+1];}
      else if(word(12,"VP8 ") && bytes[23]===0x9d && bytes[24]===1 && bytes[25]===0x2a)size=[view.getUint16(26,true)&16383,view.getUint16(28,true)&16383];
      else if(word(12,"VP8L") && bytes[20]===0x2f)size=[1+((bytes[22]&63)<<8|bytes[21]),1+((bytes[24]&15)<<10|bytes[23]<<2|bytes[22]>>6)];
    }
    return size && size.every(value=>value>0 && value<=2048)?size:null;
  }
  async function sanitize(parts,total,type,signal) {
    const bytes=new Uint8Array(total);let offset=0;for(const part of parts){bytes.set(part,offset);offset+=part.length;}
    const size=dimensions(bytes,type);if(!size || signal.aborted)throw new Error();
    const bitmap=await createImageBitmap(new Blob([bytes],{type}));
    try {
      if(signal.aborted || bitmap.width!==size[0] || bitmap.height!==size[1])throw new Error();
      const canvas=document.createElement("canvas"),context=canvas.getContext("2d",{alpha:false});if(!context)throw new Error();
      try {
        for(const [width,height] of [[320,240],[240,180],[160,120],[80,60]]) {
          if(signal.aborted)throw new Error();
          const scale=Math.min(1,width/size[0],height/size[1]);canvas.width=Math.max(1,Math.floor(size[0]*scale));canvas.height=Math.max(1,Math.floor(size[1]*scale));
          context.drawImage(bitmap,0,0,canvas.width,canvas.height);
          const blob=await new Promise(resolve=>canvas.toBlob(resolve,"image/png"));
          if(signal.aborted)throw new Error();
          if(blob && blob.size>0 && blob.size<=MAX_OUTPUT) {
            const output=new Uint8Array(await blob.arrayBuffer());if(signal.aborted || output.length>MAX_OUTPUT)throw new Error();return output;
          }
        }
        throw new Error();
      } finally {canvas.width=canvas.height=1;}
    } finally {bitmap.close();}
  }
  async function invoke(encoded,privacy) {
    if(pending || typeof encoded!=="string" || encoded.length>256)return stringify({v:1,s:"unavailable"});
    let request;
    try {request=parse(encoded.slice(3));}catch(_){return stringify({v:1,s:"unavailable"});}
    if(!["g,i,k,n","g,i,k,n,q","g,i,k,n,p,q"].includes(Object.keys(request).sort().join(",")) || !Object.values(request).every(value=>Number.isSafeInteger(value)&&value>0) || request.q>1500 || request.p && request.p!==1 && request.p!==2 || encoded.startsWith("V1:") && (request.p||request.q))return stringify({v:1,s:"unavailable"});
    const current=target(request);if(!current)return stringify({v:1,s:"unavailable"});
    if(encoded.startsWith("V1:"))return stringify(crop(request,privacy));
    const url=current.url;
    if(!url || url.length>4096)return stringify(crop(request,privacy));
    let parsed;try {parsed=new URL(url,document.baseURI);}catch(_){return stringify(crop(request,privacy));}
    if(!["https:","http:","blob:","data:"].includes(parsed.protocol))return stringify(crop(request,privacy));
    if(request.p===2 && (!["https:","http:"].includes(parsed.protocol) || parsed.origin===location.origin))return stringify({v:1,s:"unavailable"});
    const abort=new AbortController();pending=abort;let headers=false;
    const timer=setTimeout(()=>abort.abort(),request.q||1500);
    try {
      const response=await fetchAsset(url,{signal:abort.signal,credentials:request.p===2?"omit":current.node.crossOrigin==="anonymous"?"same-origin":"include",mode:"cors",redirect:"error",cache:"force-cache"});
      headers=true;const type=(response.headers.get("content-type")||"").split(";")[0].trim().toLowerCase();
      if(!response.ok || !["image/png","image/jpeg","image/webp"].includes(type) ||
         Number(response.headers.get("content-length")||0)>MAX_RAW || !response.body)throw new Error();
      const reader=response.body.getReader(),parts=[];let total=0;
      while(true){const item=await reader.read();if(item.done)break;total+=item.value.length;if(total>MAX_RAW){abort.abort();throw new Error();}parts.push(item.value);}
      if(!target(request) || abort.signal.aborted)throw new Error();
      const output=await sanitize(parts,total,type,abort.signal);
      if(!target(request) || abort.signal.aborted)throw new Error();
      let binary="";for(const value of output)binary+=String.fromCharCode(value);
      return stringify({v:1,s:"fetch",i:request.i,g:request.g,k:request.k,n:request.n,bytes:encode(binary)});
    } catch(_) {
      if(request.p===1 && !headers && !abort.signal.aborted && ["https:","http:"].includes(parsed.protocol) && parsed.origin!==location.origin && target(request))return stringify({v:1,s:"retry",i:request.i,g:request.g,k:request.k,n:request.n});
      return stringify(crop(request,privacy));
    }
    finally {clearTimeout(timer);pending=null;}
  }
  Object.defineProperty(globalThis,NAME,{value:Object.freeze({evidence,invoke}),writable:false,configurable:false});
})();
