// Immutable isolated-world pixel privacy survey. No values leave this world.
(() => {
  "use strict";
  const NAME = "__zephiumPixelPrivacyV1";
  if (Object.hasOwn(globalThis, NAME)) return;
  const stringify = JSON.stringify, getStyle = getComputedStyle;
  const rect = Element.prototype.getBoundingClientRect;
  const rangeRect = Range.prototype.getBoundingClientRect;
  const rangeSelect = Range.prototype.selectNodeContents;
  const inputValue = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").get;
  const areaValue = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value").get;
  const values = new WeakMap(), roots = new WeakSet();
  let revision = 1, exhausted = false, rootCount = 0;
  function changed() {
    if (revision >= Number.MAX_SAFE_INTEGER) exhausted = true;
    else revision += 1;
  }
  const observer = new MutationObserver(changed);
  function watch(root) {
    if (roots.has(root)) return;
    if (++rootCount > 256) {exhausted=true;observer.disconnect();return;}
    roots.add(root);
    observer.observe(root, {subtree:true, childList:true, attributes:true, characterData:true});
  }
  watch(document);
  for (const type of ["input", "change", "scroll", "resize", "load", "transitionrun", "animationstart"])
    addEventListener(type, changed, true);
  function probe(secret) {
    let clear = !exhausted;
    const w=innerWidth, h=innerHeight, x=scrollX, y=scrollY;
    try {
      if (observer.takeRecords().length) changed();
      if (!Number.isFinite(w+h+x+y) || w<=0 || h<=0 || w>16384 || h>16384) clear=false;
      const animations=document.getAnimations();
      if (animations.length>128 || animations.some(item=>item.playState==="running" || item.pending)) clear=false;
      const stack=[document.documentElement];
      let visited=0, textBytes=0, tail="", spaced="", attributeTail="", attributeSpaced="";
      const range=document.createRange();
      const inspect=(text,attribute=false)=>{
        if(!text)return true;
        if(text.length>4096)return false;
        for(const character of text){const point=character.codePointAt(0);textBytes+=point<=127?1:point<=2047?2:point<=65535?3:4;}
        if(textBytes>65536 || secret(null,text,true))return false;
        if(attribute) {
          attributeTail=(attributeTail+text).slice(-4096);attributeSpaced=(attributeSpaced+" "+text).slice(-4096);
          return !secret(null,attributeTail) && !secret(null,attributeSpaced);
        }
        tail=(tail+text).slice(-4096);spaced=(spaced+" "+text).slice(-4096);
        return !secret(null,tail) && !secret(null,spaced);
      };
      while (stack.length && clear) {
        const node=stack.pop();
        if (!node || ++visited>20000) {clear=false; break;}
        if (node.nodeType===3) {
          const parent=node.parentElement || node.getRootNode()?.host;
          if (!parent) {clear=false;break;}
          rangeSelect.call(range,node);
          const b=rangeRect.call(range), s=getStyle(parent);
          if (b.bottom<=0 || b.right<=0 || b.top>=h || b.left>=w || s.visibility!=="visible" || s.display==="none") continue;
          if (!inspect(node.nodeValue)) clear=false;
          continue;
        }
        if (node.nodeType!==1) continue;
        const b=rect.call(node), s=getStyle(node);
        const visible=b.width>0 && b.height>0 && b.bottom>0 && b.right>0 && b.top<h && b.left<w && s.visibility==="visible" && s.display!=="none";
        const tag=node.localName;
        if(visible) for(const name of ["aria-label","title","alt","placeholder"]) {
          if(!inspect(node.getAttribute(name)||"",true)) {clear=false;break;}
        }
        if(!clear)break;
        if (visible && (["iframe","frame","object","embed"].includes(tag) || tag.includes("-") && !node.shadowRoot || secret(node,null))) {clear=false; break;}
        // Known credential controls can be moved, filled, and restored by page
        // script while native capture is pending. Exclude them even offscreen.
        if (["input","textarea","select"].includes(tag) && secret(node,null)) {clear=false; break;}
        if (tag==="input" || tag==="textarea") {
          const value=(tag==="input" ? inputValue : areaValue).call(node);
          if (value.length>4096) {clear=false; break;}
          if (values.has(node) && values.get(node)!==value) changed();
          values.set(node,value);
          if (visible && !inspect(value)) {clear=false; break;}
        }
        if (node.shadowRoot) {
          watch(node.shadowRoot);
          for (let at=node.shadowRoot.childNodes.length-1;at>=0;at--) stack.push(node.shadowRoot.childNodes[at]);
        }
        // Preserve document order for secrets split across adjacent text nodes.
        for (let at=node.childNodes.length-1;at>=0;at--) stack.push(node.childNodes[at]);
        if (stack.length>20000) {clear=false; break;}
      }
      if (observer.takeRecords().length) {changed(); clear=false;}
    } catch (_) {clear=false;}
    return stringify({v:1,r:revision,c:clear && !exhausted,w,h,x,y});
  }
  Object.defineProperty(globalThis, NAME, {value:Object.freeze({probe}), writable:false, configurable:false});
})();
