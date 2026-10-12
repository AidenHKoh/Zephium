import {readFileSync} from 'node:fs';
import {runInNewContext} from 'node:vm';
import {strict as assert} from 'node:assert';

class Element {
  constructor(tag, attrs={}, box={x:0,y:0,width:800,height:600}) {
    this.localName=tag;this.attrs=attrs;this.box=box;this.nodeType=1;this.childNodes=[];
    this.parentNode=null;this.currentSrc=attrs.src||'';this.complete=true;this.naturalWidth=400;
  }
  append(child){child.parentNode=this;this.childNodes.push(child);return child;}
  get children(){return this.childNodes.filter(node=>node.nodeType===1);}
  get parentElement(){return this.parentNode?.nodeType===1?this.parentNode:null;}
  get isConnected(){return this.getRootNode().nodeType===9;}
  getRootNode(){let node=this;while(node.parentNode)node=node.parentNode;return node;}
  getAttribute(key){return this.attrs[key]??null;}
  hasAttribute(key){return Object.hasOwn(this.attrs,key);}
  get content(){return this.attrs.content||'';}
  get textContent(){return this.childNodes.map(node=>node.nodeType===3?node.nodeValue:node.textContent).join('');}
  set textContent(text){this.childNodes=[{nodeType:3,nodeValue:text,parentNode:this}];}
  contains(node){for(;node;node=node.parentNode)if(node===this)return true;return false;}
  getBoundingClientRect(){return {...this.box,right:this.box.x+this.box.width,bottom:this.box.y+this.box.height};}
  querySelector(){return null;}
}
const asset=readFileSync(new URL('../crates/zephium-agentic/assets/semantic-record-media-v1.js',import.meta.url),'utf8');
assert.equal(asset.split('Object.freeze({evidence,invoke})').length,2);
// Expose only the private revalidation predicate in this synthetic process.
const program=asset.replace('Object.freeze({evidence,invoke})','Object.freeze({evidence,invoke,target})');
const page='https://catalog.example.test/product?id=7&variant=blue';
function fixture({other=false, metadata=true, duplicate=false, changedUrl=false, responsive=false, pageImage=false}={}) {
  const document={nodeType:9,baseURI:page,URL:page};
  const html=new Element('html');html.parentNode=document;document.documentElement=html;
  const head=html.append(new Element('head'));
  const body=html.append(new Element('body'));
  const main=body.append(new Element('main'));
  const product=main.append(new Element('div'));
  const details=product.append(new Element('div',{}, {x:420,y:0,width:380,height:80}));
  const heading=details.append(new Element('h1',{}, {x:430,y:10,width:350,height:30}));heading.textContent='Orbital telescope';
  const gallery=product.append(new Element('div',{}, {x:0,y:0,width:400,height:400}));
  const photo=gallery.append(new Element('img',{src:'https://images.example.test/orbital.webp',alt:''}, {x:0,y:0,width:400,height:350}));
  const counter=gallery.append(new Element('p',{}, {x:0,y:360,width:100,height:25}));counter.textContent=other?'Another product':'1 / 4';
  const script=head.append(new Element('script',{type:'application/ld+json'}));
  const item={'@type':'Product',name:other?'Another product':'Orbital telescope',url:changedUrl?'https://catalog.example.test/product?id=8':page,image:photo.currentSrc};
  if(responsive){photo.currentSrc='https://images.example.test/orbital-small.webp';photo.attrs.src=photo.currentSrc;photo.attrs.srcset='https://images.example.test/orbital.webp 2x';}
  script.textContent=JSON.stringify(duplicate?[item,{...item,sku:'conflicting'}]:item);
  Object.defineProperty(document,'scripts',{get:()=>head.children.filter(node=>node.localName==='script')});
  if(pageImage){photo.localName='meta';photo.attrs={property:'og:image',content:'https://images.example.test/orbital-og.webp'};}
  const records=[
    {element:heading,media:null,sensitivity:'public',wire:{k:1,r:'heading',l:1,n:'Orbital telescope',b:{x:430,y:10,w:350,h:30}}},
    {element:photo,media:pageImage?'page_image':'image',sensitivity:'public',wire:{k:2,r:'image',b:{x:0,y:0,w:400,h:350}}},
    {element:counter,media:null,sensitivity:'public',wire:{k:3,r:'paragraph',t:counter.textContent,b:{x:0,y:360,w:100,h:25}}},
    ...(metadata?[{element:script,media:null,sensitivity:'public',wire:{k:4,r:'paragraph',n:'Page facts',t:'Product metadata'}}]:[])
  ];
  const context={Element,document,WeakRef,WeakMap,Map,Set,URL,JSON,AbortController,innerWidth:1000,innerHeight:800,
    location:{origin:new URL(page).origin},setTimeout,clearTimeout,addEventListener(){},fetch(){throw Error('no network in structural fixture');},btoa(){throw Error('no pixels in structural fixture');},
    getComputedStyle(){return {display:'block',visibility:'visible',backgroundImage:'none',content:''};}};
  runInNewContext(program,context);
  const api=context.__zephiumRecordMediaV1;
  const helpers={apply:Reflect.apply,read:(get,node)=>get.call(node),nodeContains:Element.prototype.contains,
    elementRect:node=>node.getBoundingClientRect(),tagName:node=>node.localName,nodeType:node=>node.nodeType,
    nodeParentGetter:function(){return this.parentNode;},imageCompleteGetter:function(){return this.complete;},
    imageNaturalWidthGetter:function(){return this.naturalWidth;},pageTitleText:()=>null};
  const rows=api.evidence(records,1,1,helpers);
  return {api,rows,heading,photo,script,document,head,item};
}
const good=fixture();
assert(good.rows.find(row=>row.k===2)?.r.includes(1),'a title panel and gallery of the declared page entity lost their association');
assert(good.api.target({i:1,g:1,k:2,n:1}),'fresh exact entity proof was not retained');
good.script.textContent=JSON.stringify({'@type':'Product',name:'Another product',url:page,image:good.photo.currentSrc});
assert.equal(good.api.target({i:1,g:1,k:2,n:1}),null,'changed metadata reused an old entity proof');
for(const options of [{other:true},{metadata:false},{duplicate:true},{changedUrl:true}]) {
  const negative=fixture(options);
  assert(!negative.rows.find(row=>row.k===2)?.r.includes(1),'an unrelated or ambiguous image crossed a record boundary');
}
const renamed=fixture();renamed.heading.textContent='Another product';
assert.equal(renamed.api.target({i:1,g:1,k:2,n:1}),null,'renamed heading reused image authority');
const added=fixture();
const extra=added.head.append(new Element('script',{type:'application/ld+json'}));extra.textContent=JSON.stringify(added.item);
assert.equal(added.api.target({i:1,g:1,k:2,n:1}),null,'a new conflicting entity escaped fresh revalidation');
const redirected=fixture();redirected.document.URL=page.replace('blue','red');
assert.equal(redirected.api.target({i:1,g:1,k:2,n:1}),null,'a changed full-query address reused metadata authority');
const responsive=fixture({responsive:true});
assert(responsive.rows.find(row=>row.k===2)?.r.includes(1),'an explicitly declared responsive image lost its entity relation');
assert(responsive.api.target({i:1,g:1,k:2,n:1}),'responsive image proof was not retained');
delete responsive.photo.attrs.srcset;
assert.equal(responsive.api.target({i:1,g:1,k:2,n:1}),null,'a removed responsive-source relation remained valid');
const tooLarge=fixture();tooLarge.script.textContent=' '.repeat(262145);
assert.equal(tooLarge.api.target({i:1,g:1,k:2,n:1}),null,'oversized metadata escaped the byte bound');
const tooMany=fixture();tooMany.script.textContent=JSON.stringify(Array.from({length:401},()=>({})));
assert.equal(tooMany.api.target({i:1,g:1,k:2,n:1}),null,'exhausted metadata traversal kept an image proof');
const extraScripts=fixture();for(let i=0;i<129;i++)extraScripts.head.append(new Element('script'));
assert.equal(extraScripts.api.target({i:1,g:1,k:2,n:1}),null,'exhausted script inventory kept an image proof');
const declared=fixture({pageImage:true});
assert(declared.rows.find(row=>row.k===2)?.r.includes(1),'observed page image lost its exact declared entity relation');
assert(declared.api.target({i:1,g:1,k:2,n:1}),'page-image entity proof was not retained');
declared.photo.attrs.content='https://images.example.test/unrelated.webp';
assert.equal(declared.api.target({i:1,g:1,k:2,n:1}),null,'changed page-image URI reused an entity proof');
const ambiguous=fixture({pageImage:true});
ambiguous.script.textContent=JSON.stringify([ambiguous.item,{...ambiguous.item,name:'Another item on this page'}]);
assert.equal(ambiguous.api.target({i:1,g:1,k:2,n:1}),null,'two entities sharing a page address retained the page-image authority');
const withoutPageIdentity=fixture({pageImage:true,changedUrl:true});
assert(!withoutPageIdentity.rows.find(row=>row.k===2)?.r.includes(1),'a page image attached to an entity from another full-query address');
console.log('record media structural and metadata custody fixtures passed');

const cors=fixture({pageImage:true});
const invoke=(prefix,extra='')=>cors.api.invoke(`${prefix}:{"i":1,"g":1,"k":2,"n":1${extra}}`,()=>{throw Error('no capture authority');}).then(JSON.parse);
assert.equal((await invoke('M1',',"q":1000')).s,'unavailable','session-only pages must not offer a public fallback');
assert.equal((await invoke('M1',',"p":1,"q":1000')).s,'retry');
const nativeSource=await invoke('M1',',"p":2,"q":1000');
assert.equal(nativeSource.s,'public_asset');
assert.equal(nativeSource.u,'https://images.example.test/orbital-og.webp');
assert.equal((await invoke('U1')).u,nativeSource.u);
assert.equal((await invoke('U1',',"q":1000')).s,'unavailable');
assert.equal((await invoke('X1')).s,'unavailable');
cors.photo.attrs.content='https://images.example.test/changed.webp';
assert.equal((await invoke('U1')).s,'unavailable','changed source cannot publish fetched bytes');
console.log('record image CORS fallback and exact-source revalidation fixtures passed');
