import test from 'node:test'
import assert from 'node:assert/strict'
import {readFileSync} from 'node:fs'
import {ref,computed} from 'vue'

const source=readFileSync(new URL('./SystemPrompts.vue',import.meta.url),'utf8')
const script=source.match(/<script setup>([\s\S]*?)<\/script>/)[1].replace(/^import .*\n/gm,'')
function editor(){
 const requests=[]
 const state=new Function('ref','computed','onMounted','defineProps','ElMessage','ElMessageBox',script+'\nreturn {edit,draft,useDefault,save,close,selected}')(
  ref,computed,()=>{},()=>({request:async(path,options)=>{requests.push(JSON.parse(options.body));return {prompts:[],directory:'/instance/conf'}}}),{success:()=>{}},{confirm:async()=>{}}
 )
 return {...state,requests}
}
test('saving inherited prompts requests removal, not a copy of default text',async()=>{
 const view=editor()
 view.edit({id:'discussion',content:'release instructions',default:'release instructions',overridden:false})
 assert.equal(view.useDefault.value,true)
 await view.save()
 assert.equal(view.requests[0].reset,true)
})
test('saving an override is explicit even when its text equals the default',async()=>{
 const view=editor()
 view.edit({id:'discussion',content:'release instructions',default:'release instructions',overridden:false})
 view.useDefault.value=false
 await view.save()
 assert.equal(view.requests[0].reset,false)
 assert.equal(view.requests[0].expected,'release instructions')
})
test('restoring an override uses reset and preserves the optimistic read value',async()=>{
 const view=editor()
 view.edit({id:'discussion',content:'custom instructions',default:'release instructions',overridden:true})
 assert.equal(view.useDefault.value,false)
 view.useDefault.value=true;view.draft.value='release instructions'
 await view.save()
 assert.equal(view.requests[0].reset,true)
 assert.equal(view.requests[0].expected,'custom instructions')
})
