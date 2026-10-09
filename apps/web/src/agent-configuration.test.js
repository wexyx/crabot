import test from 'node:test'
import assert from 'node:assert/strict'
import {readFileSync} from 'node:fs'
import {ref,computed} from 'vue'

const source=readFileSync(new URL('./AgentManagement.vue',import.meta.url),'utf8')
const defaultResponse=readFileSync(new URL('../../../conf/response.md',import.meta.url),'utf8').trimEnd()
// Exercise the editor's actual state and request payload without mounting Element Plus.
const script=source.match(/<script setup>([\s\S]*?)<\/script>/)[1].replace(/^import .*\n/gm,'')
function editor(selectedAgent){
 const requests=[]
 const props={project:'test',selectedAgent,request:async(path,options)=>{
  if(options)requests.push(JSON.parse(options.body))
  return {agents:[],mounts:[]}
 }}
 const noop=()=>{}
 const state=new Function('ref','computed','watch','onMounted','onBeforeUnmount','defineProps','defineEmits','ElMessage','defaultResponse',
  script+'\nreturn {edit,draft,useInstance,inheritsInstance,save,data}')(
  ref,computed,noop,noop,noop,()=>props,()=>noop,{success:noop},defaultResponse)
 return {...state,requests}
}

test('only Crabot offers instance inheritance',()=>{
 assert.ok(source.includes('<el-checkbox v-if="draft.provider===\'crabot\'" v-model="useInstance">'))
 assert.ok(source.includes('<p v-if="inheritsInstance"'))
 assert.ok(source.includes('<RuntimeConfigurationFields v-else'))
 const e=editor()
 e.edit(null)
 assert.equal(e.inheritsInstance.value,true)
 for(const provider of ['codex','claude','opencode','mock']){
  e.draft.value.provider=provider
  assert.equal(e.inheritsInstance.value,false,provider)
 }
 e.draft.value.provider='crabot'
 assert.equal(e.inheritsInstance.value,true)
 e.useInstance.value=false
 e.draft.value.provider='claude'
 e.draft.value.provider='crabot'
 assert.equal(e.inheritsInstance.value,false,'switching preserves the Crabot choice')
})

for(const provider of ['codex','claude','opencode','mock']){
 test(`${provider} saves explicit configuration even when the old inheritance flag is true`,async()=>{
  const e=editor()
  e.edit({id:'worker',provider,configuration:{inherits_instance:true}})
  e.draft.value.configuration.AGENT_ENV_JSON='{"CUSTOM":"value"}'
  e.draft.value.configuration.CODEX_BIN='codex --model test'
  e.draft.value.configuration.CLAUDE_BIN='claude --model sonnet'
  e.draft.value.configuration.OPENCODE_BIN='opencode'
  const expected={...e.draft.value.configuration}
  await e.save()
  assert.deepEqual(e.requests[0].configuration,expected)
  assert.equal(e.requests[0].provider,provider)
 })
}

test('Crabot can still inherit or save its own model configuration',async()=>{
 const e=editor()
 e.edit(null)
 await e.save()
 assert.equal(e.requests[0].configuration,null)
 e.edit(null)
 e.useInstance.value=false
 e.draft.value.configuration.MODEL_NAME='private-model'
 await e.save()
 assert.equal(e.requests[1].configuration.MODEL_NAME,'private-model')
})

test('editing the default Agent role does not overwrite its runtime configuration',async()=>{
 const e=editor('default')
 const agent={id:'default',provider:'claude',is_default:true}
 e.data.value.agents=[agent]
 e.edit(agent)
 await e.save()
 assert.equal(Object.hasOwn(e.requests[0],'configuration'),false)
})
