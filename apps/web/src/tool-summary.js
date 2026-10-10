import {toolCodeCall} from './tool-code-preview.js'
const limit=56
const clean=value=>typeof value==='string'?value.replace(/[\x00-\x1f\x7f]/g,' ').replace(/\s+/g,' ').trim():''
const shorten=value=>{const chars=Array.from(clean(value));return chars.length>limit?chars.slice(0,limit-1).join('')+'…':chars.join('')}
const object=value=>{try{return typeof value==='string'?JSON.parse(value):value}catch{return null}}
const sensitive=value=>/token|password|passwd|secret|api[_-]?key|authorization/i.test(value)

function commandPreview(command){
 const words=clean(command).split(' '),result=[]
 let hideNext=false
 for(const word of words){
  if(hideNext){result.push('***');hideNext=false;continue}
  // Headers, request bodies and inline programs belong in expanded details.
  if(['-H','--header','-d','--data','--data-raw','-c','--command','-e','--eval'].includes(word)){result.push(word,'…');break}
  const at=word.indexOf('=')
  if(at>0&&!word.includes('://')&&sensitive(word.slice(0,at))){result.push(word.slice(0,at+1)+'***');continue}
  if(sensitive(word)&&word.startsWith('-')){result.push(word);hideNext=true;continue}
  if(word==='-u'||word==='--user'){result.push(word);hideNext=true;continue}
  if(word.includes('://')){
   const [scheme,...rest]=word.split('://')
   let address=rest.join('://').split('?')[0].split('#')[0]
   if(address.includes('@'))address='***@'+address.slice(address.lastIndexOf('@')+1)
   result.push(scheme+'://'+address);continue
  }
  const path=word.replace(/^['"]|['"]$/g,'')
  if(path.startsWith('/')&&path!=='/'){
   const parts=path.split('/').filter(Boolean)
   result.push(result.length===0?parts.at(-1):parts.length>2?'…/'+parts.slice(-2).join('/'):path)
  }else result.push(word)
 }
 return result.join(' ')
}

export function toolSummary(name,input){
 name=clean(name)||'tool';input=object(input)||{}
 let action=''
 if(name==='execute'&&typeof input.code==='string'){
  const call=toolCodeCall(input.code)
  action=call?toolSummary(call.name,call.input):'代码执行'
 }
 else if(['shell','execute','exec_command','bash','Bash','command_execution'].includes(name)){
  const command=input.command??input.cmd
  action=typeof command==='string'?commandPreview(command):({read:'读取进程输出',write:'发送进程输入',stop:'停止进程'}[input.action]||'')
 }
 else if(['find','search'].includes(name)){
  const target=clean(input.target).toLowerCase()
  const subject=clean(input.id)||clean(input.name)||clean(input.query)
  action=subject?`${target?target+' : ':''}${JSON.stringify(subject)}`:target
 }
 return shorten(action?`${name} · ${action}`:name)
}

export function toolEventSummary(row){
 const event=row?.payload||row||{}
 const data=object(event.content??event.text)
 return toolSummary(event.name||data?.name,event.input??event.arguments??data?.input??data?.arguments)
}
