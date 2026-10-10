// Display-only lexical scan. Never evaluate provider code or interpolate variables.
export function toolCodeCall(code){
 if(typeof code!=='string'||new TextEncoder().encode(code).length>65536)return null
 const tokens=[]
 for(let i=0;i<code.length;){
  let c=code[i++]
  if(/\s/.test(c))continue
  if(c==='/'&&code[i]==='/'){while(i<code.length&&code[i++]!=='\n'){};continue}
  if(c==='/'&&code[i]==='*'){i++;while(i<code.length&&!(code[i-1]==='*'&&code[i]==='/'))i++;i++;continue}
  if(['"',"'",'`'].includes(c)){
   let text='',valid=c!=='`',closed=false
   while(i<code.length){
    const next=code[i++]
    if(next===c){closed=true;break}
    if(next==='\\'){
     const escaped=code[i++]
     if(['\\',"'",'"','/'].includes(escaped))text+=escaped
     else if(['n','r','t'].includes(escaped))text+=({n:'\n',r:'\r',t:'\t'})[escaped]
     else valid=false
    }else text+=next
   }
   tokens.push({kind:valid&&closed?'text':'mark',value:valid&&closed?text:'?'});continue
  }
  if(/[A-Za-z_$]/.test(c)){while(i<code.length&&/[A-Za-z0-9_$]/.test(code[i]))c+=code[i++];tokens.push({kind:'word',value:c})}
  else tokens.push({kind:'mark',value:c})
 }
 const is=(i,kind,value)=>tokens[i]?.kind===kind&&tokens[i]?.value===value
 for(let at=0;at+5<tokens.length;at++){
  if(!is(at,'word','tools')||!is(at+1,'mark','.')||tokens[at+2]?.kind!=='word'||!is(at+3,'mark','(')||!is(at+4,'mark','{'))continue
  const name=tokens[at+2].value.replace(/^crabot_tool_/,'')
  if(!name||name.length>64)continue
  const input={};let depth=1
  for(let i=at+5;i<tokens.length;i++){
   const t=tokens[i]
   if(t.kind==='mark'&&['{','[','('].includes(t.value))depth++
   else if(t.kind==='mark'&&['}',']',')'].includes(t.value)){if(--depth===0)break}
   else if(depth===1&&['word','text'].includes(t.kind)&&['target','query','id','name','command','cmd','action'].includes(t.value)&&is(i+1,'mark',':')&&tokens[i+2]?.kind==='text'&&(is(i+3,'mark',',')||is(i+3,'mark','}')))input[t.value]=tokens[i+2].value
  }
  return {name,input}
 }
 return null
}
