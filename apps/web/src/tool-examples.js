// Examples are editable hints, never an instruction to execute automatically.
export function exampleArguments(schema={},name=''){
 const root=schema
 function sample(s,key='',depth=0){
  if(depth>8)return null
  if(s?.$ref?.startsWith('#/')){s=s.$ref.slice(2).split('/').reduce((v,k)=>v?.[k.replace(/~1/g,'/').replace(/~0/g,'~')],root)||{}}
  if(Object.hasOwn(s,'default'))return structuredClone(s.default)
  if(s.examples?.length)return structuredClone(s.examples[0])
  if(Object.hasOwn(s,'const'))return structuredClone(s.const)
  if(s.enum?.length)return structuredClone(s.enum[0])
  if(s.oneOf||s.anyOf)return sample((s.oneOf||s.anyOf).find(v=>v.type!=='null')||{},key,depth+1)
  const type=Array.isArray(s.type)?s.type.find(v=>v!=='null'):s.type
  if(type==='object'||s.properties)return Object.fromEntries(Object.entries(s.properties||{}).map(([k,v])=>[k,sample(v,k,depth+1)]))
  if(type==='array')return s.minItems>0?Array.from({length:Math.min(s.minItems,5)},()=>sample(s.items||{},key,depth+1)):[]
  if(type==='integer'||type==='number')return s.minimum??(typeof s.exclusiveMinimum==='number'?s.exclusiveMinimum+1:1)
  if(type==='boolean')return false
  if(type==='null')return null
  if(key==='command')return 'pwd'
  if(key==='code')return 'print("Hello, Crabot")'
  if(key==='path')return './README.md'
  if(key==='url')return 'https://example.com'
  if(key==='from_line')return 1
  if(key==='to_line')return 20
  return s.description?`请填写：${s.description.slice(0,70)}`:`请填写 ${key||'内容'}`
 }
 return sample(schema)||{}
}
