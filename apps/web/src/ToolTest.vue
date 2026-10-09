<script setup>
import {ref,onBeforeUnmount,computed} from 'vue'
import {exampleArguments} from './tool-examples.js'
const emit=defineEmits(['completed'])
const props=defineProps({request:Function,project:String,group:String,scope:String,agent:String,name:String,initialArguments:Object,schema:Object,disabled:Boolean})
const example=computed(()=>props.initialArguments??exampleArguments(props.schema||{type:'object',properties:{args:{type:'array',items:{type:'string'}}}},props.name))
const args=ref(JSON.stringify(example.value,null,2)),run=ref(null),error=ref(''),submitting=ref(false),approvals=ref([]),deciding=ref(false)
let timer,alive=true,polling=false
const active=computed(()=>run.value?.status==='running')
const managementApproval=computed(()=>run.value?.output?.status==='pending'?run.value.output:null)
const labels={running:'执行中',completed:'已返回',failed:'失败',cancelled:'已取消'}
async function poll(){if(polling||!alive||!run.value)return;polling=true;try{
 const result=await props.request('/v1/repl/'+props.project+'/tool-tests/'+run.value.id)
 if(!alive)return;run.value=result
 if(result.status==='running'){
  const pending=await props.request('/v1/workspace/approvals');if(alive)approvals.value=pending.requests.filter(a=>a.correlation_id===result.id)
 }else{clearInterval(timer);approvals.value=[];if(result.status==='completed')emit('completed',result.output)}
 }catch(e){if(alive)error.value=e.message}finally{polling=false}}
async function start(){error.value='';let argumentsValue;try{argumentsValue=JSON.parse(args.value);if(!argumentsValue||Array.isArray(argumentsValue)||typeof argumentsValue!=='object')throw new Error('参数必须是 JSON 对象。')}catch(e){error.value=e.message;return}submitting.value=true;try{const row=await props.request('/v1/repl/'+props.project+'/tool-config/'+props.scope+'/'+encodeURIComponent(props.agent)+'/tests',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({name:props.name,arguments:argumentsValue,group:props.group||undefined})});if(!alive){props.request('/v1/repl/'+props.project+'/tool-tests/'+row.id,{method:'DELETE'}).catch(()=>{});return}run.value=row;await poll();if(active.value)timer=setInterval(poll,600)}catch(e){error.value=e.message}finally{submitting.value=false}}
async function cancel(){try{await props.request('/v1/repl/'+props.project+'/tool-tests/'+run.value.id,{method:'DELETE'});await poll()}catch(e){error.value=e.message}}
async function decide(item,allow,management=false){deciding.value=true;try{const result=await props.request(management?'/v1/admin-agent/'+props.project+'/approvals/'+item.id:'/v1/workspace/approvals/'+item.id,{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({allow})});if(management)run.value={...run.value,output:result};else await poll()}catch(e){error.value=e.message}finally{deciding.value=false}}
onBeforeUnmount(()=>{alive=false;clearInterval(timer);if(active.value)props.request('/v1/repl/'+props.project+'/tool-tests/'+run.value.id,{method:'DELETE'}).catch(()=>{})})
</script>
<template>
 <section class="test-console"><div class="section-heading inline"><h4>试运行 <code>{{name}}</code></h4><span class="status-pill" :class="{amber:active}">{{run?labels[run.status]:'尚未运行'}}</span></div><p class="test-warning">真实执行，可能修改数据或发起网络请求。直接调用工具，不绕过权限；部分管理工具可能触发 Agent 任务。离开此测试区会取消未结束的测试。</p><p class="muted">已填入参数示例，请替换文件路径、ID 等占位内容后运行。<span v-if="schema?.required?.length">必填：{{schema.required.join('、')}}</span></p><el-button text :disabled="active||submitting" @click="args=JSON.stringify(example,null,2)">恢复示例</el-button><label class="field-stack">输入参数<el-input type="textarea" class="code-editor" v-model="args" rows="4" spellcheck="false" :disabled="active||submitting" aria-label="试运行参数"></el-input></label><div class="editor-actions"><el-button type="default" native-type="button" v-if="active" class="secondary" :disabled="submitting" @click="cancel">停止测试</el-button><el-button type="primary" native-type="button" v-else :disabled="disabled||submitting||!agent" @click="start">{{submitting?'启动中…':'运行测试'}}</el-button></div><p v-if="error" role="alert" class="config-alert danger">{{error}}</p>
 <el-alert type="warning" :closable="false" show-icon v-for="item in approvals" :key="item.id" class="approval"><b>需要你确认本次操作</b><p>{{item.operation}}</p><p>工作目录：{{item.workdir}}</p><pre>{{item.command||item.path}}</pre><el-button type="primary" native-type="button" :disabled="deciding" @click="decide(item,true)">允许一次</el-button><el-button type="default" native-type="button" class="secondary" :disabled="deciding" @click="decide(item,false)">拒绝</el-button></el-alert>
 <el-alert type="warning" :closable="false" show-icon v-if="managementApproval" class="approval"><b>管理操作需要确认 · {{managementApproval.tool}}</b><p>{{managementApproval.warning||managementApproval.client_id}}</p><pre>{{JSON.stringify(managementApproval.input||{},null,2)}}</pre><el-button type="primary" native-type="button" :disabled="deciding" @click="decide(managementApproval,true,true)">确认执行</el-button><el-button type="default" native-type="button" class="secondary" :disabled="deciding" @click="decide(managementApproval,false,true)">拒绝</el-button></el-alert>
 <div v-if="run" class="test-result"><div class="file-editor-bar"><b>执行结果</b><small>{{run.duration_ms===undefined?'等待输出…':run.duration_ms+' ms'}} · {{run.id.slice(0,8)}}</small></div><pre>{{run.error|| (run.output===null?'等待执行完成或用户确认…':JSON.stringify(run.output,null,2))}}</pre></div>
 </section>
</template>
