<script setup>
import {defaultResponse} from './default-response.js'
import {ref,computed,watch,onMounted,onBeforeUnmount} from 'vue'
import ConfigPanel from './ConfigPanel.vue'
import GroupConfiguration from './GroupConfiguration.vue'
import {ElMessage} from 'element-plus/es/components/message/index'
import 'element-plus/es/components/message/style/css'
import AdminConfiguration from './AdminConfiguration.vue'
import RuntimeConfigurationFields from './RuntimeConfigurationFields.vue'
import AgentTestChat from './AgentTestChat.vue'
const props=defineProps({request:Function,stream:Function,project:String,selectedAgent:String})
const emit=defineEmits(['close','select','changed'])
const data=ref({agents:[],mounts:[]}),error=ref(''),notice=ref(''),busy=ref(false),draft=ref(null),pending=ref(null),virtual=ref(null),testing=ref(false),deleting=ref(false)
const creationKind=ref('local')
function chooseCreation(kind){draft.value=null;virtual.value=null;if(kind==='virtual')editVirtual(null);else edit(null)}
const editingOpen=computed({get:()=>!!draft.value||(!props.selectedAgent&&!!virtual.value),set:value=>{if(!value){draft.value=null;virtual.value=null}}})
function closeEditor(done){if(!busy.value){draft.value=null;virtual.value=null;done()}}
const instanceConfiguration=ref(null)
const modelInfo=computed(()=>{
 if(!agent.value||remote.value)return null
 const own=agent.value.configuration
 const inherited=agent.value.is_default||own?.inherits_instance!==false
 return {values:{...instanceConfiguration.value?.values,...(!inherited?own?.values:{})},inherited,hasKey:inherited?instanceConfiguration.value?.has_api_key:own?.has_api_key}
})
async function loadConfiguration(){try{instanceConfiguration.value=(await props.request('/v1/admin-agent/configuration')).configuration}catch(e){error.value=e.message}}
const useInstance=ref(true),modelKey=ref(''),clearModelKey=ref(false)
const inheritsInstance=computed(()=>draft.value?.provider==='crabot'&&useInstance.value)
const agent=computed(()=>data.value.agents.find(a=>a.id===props.selectedAgent))
const remote=computed(()=>agent.value?.kind==='remote')
let timer,alive=true,loading=false
const base=()=>'/v1/repl/'+props.project
const send=(body,method='POST')=>({method,headers:{'content-type':'application/json'},body:JSON.stringify(body)})
async function load(){if(loading)return;loading=true;try{const result=await props.request(base()+'/agents/candidates');if(alive)data.value=result}finally{loading=false}}
async function perform(fn){if(busy.value)return;busy.value=true;error.value='';notice.value='';try{await fn();emit('changed');load().catch(e=>{if(alive)error.value=e.message})}catch(e){error.value=e.message}finally{busy.value=false}}
function reset(){creationKind.value='local';draft.value=null;virtual.value=null;testing.value=false;deleting.value=false;error.value='';notice.value='';if(!props.selectedAgent)edit(null)}
function edit(a){if(a?.kind==='remote')return;draft.value=a?{client_id:a.id,role:a.role,provider:a.provider,expected_version:a.version}:{client_id:'',role:'',provider:'crabot',expected_version:0};draft.value.response_instructions=a?.response_instructions??defaultResponse;draft.value.configuration={MODEL_PROVIDER:'openai',MODEL_NAME:'',MODEL_API:'',MODEL_BASE_URL:'',MODEL_SYSTEM_PROMPT:'',CODEX_BIN:'',CLAUDE_BIN:'',OPENCODE_BIN:'',OPENCODE_MODEL:'',OPENCODE_AGENT:'',OPENCODE_AUTO_APPROVE:'',OPENCODE_THINKING:'',OPENCODE_STANDALONE:'',AGENT_ENV_JSON:'{}',...a?.configuration?.values};useInstance.value=a?.configuration?.inherits_instance??true;modelKey.value='';clearModelKey.value=false}
function editVirtual(a){virtual.value=a?{key:a.id,version:a.version,body:{name:a.name,role:a.role,response_instructions:a.response_instructions,policy:a.policy}}:{key:'',version:0,body:{name:'',policy:{mode:'relay',relay_strategy:'manual',members:[],leader:null,rounds:1,instructions:''}}}}
async function save(test=false){await perform(async()=>{const id=draft.value.client_id;await props.request(base()+'/agents',send({...draft.value,configuration:agent.value?.is_default?undefined:inheritsInstance.value?null:{...draft.value.configuration,...(clearModelKey.value?{MODEL_API_KEY:''}:modelKey.value?{MODEL_API_KEY:modelKey.value}:{})}},'PUT'));draft.value=null;emit('select',id);ElMessage.success({message:'Agent 已保存。',duration:2500,grouping:true});if(test===true){await load();testing.value=true}})}
async function start(){await perform(async()=>{await props.request(base()+'/agents/'+encodeURIComponent(agent.value.id)+'/start',send({}))})}
async function stop(){await perform(async()=>{pending.value=await props.request(base()+'/agents/'+encodeURIComponent(agent.value.id)+'/stop',send({}))})}
async function remove(){await perform(async()=>{await props.request(base()+'/agents/'+encodeURIComponent(agent.value.id),send({expected_version:agent.value.version},'DELETE'));deleting.value=false;emit('close')})}
async function decide(allow){await perform(async()=>{await props.request('/v1/admin-agent/'+props.project+'/approvals/'+pending.value.id,send({allow}));pending.value=null})}
watch(()=>props.selectedAgent,reset,{immediate:true})
onMounted(async()=>{loadConfiguration();await perform(load);timer=setInterval(()=>{if(!busy.value&&!document.hidden)load().catch(e=>{if(alive)error.value=e.message})},5000)})
onBeforeUnmount(()=>{alive=false;clearInterval(timer)})
</script>
<template>
<GroupConfiguration v-if="virtual&&selectedAgent" :group="virtual" :project="project" :request="request" :creating="!virtual.key" virtual-agent inline @close="virtual=null" @saved="row=>{virtual=null;perform(load);$emit('select',row.key)}"/>
<ConfigPanel v-else :title="selectedAgent?(agent?.name||agent?.id||'Agent 详情'):'新增 Agent'" inline eyebrow="AGENTS" :description="remote?'远端 Agent · 只读。请在所属 Crabot 的 Web 页面修改。':'配置、运行与测试 Agent。'" :busy="busy" :dirty="false" @close="$emit('close')">
 <div class="config-detail agent-detail">
  <div v-if="error" class="config-alert danger" role="alert">{{error}}</div><div v-if="notice" class="config-alert success">{{notice}}</div>
  <template v-if="agent">
   <div class="agent-overview"><div class="agent-identity"><span class="agent-monogram">{{(agent.name||agent.id).slice(0,1).toUpperCase()}}</span><div><span class="eyebrow">{{remote?'REMOTE AGENT':'AGENT PROFILE'}}</span><h3>{{agent.name||agent.id}}</h3><span class="status-pill" :class="{muted:!agent.online}">{{agent.online?'在线 · 可测试':'已停止'}}</span></div></div><dl class="agent-facts"><dt>名称 / ID</dt><dd>{{agent.name||agent.id}}</dd><dt>类型</dt><dd>{{agent.is_default?'默认 Agent':remote?'远端 Agent':agent.kind==='virtual'?'虚拟 Agent':'本地 Agent'}}</dd><dt>运行器</dt><dd>{{agent.provider||agent.agent_kind||'组合协作'}}</dd><template v-if="agent.provider==='crabot'&&!remote&&modelInfo"><dt>模型厂商</dt><dd>{{modelInfo.values.MODEL_PROVIDER||'未配置'}}</dd><dt>模型</dt><dd>{{modelInfo.values.MODEL_NAME||'未配置'}}</dd><dt>模型接口</dt><dd>{{modelInfo.values.MODEL_API||'厂商默认协议'}}</dd><dt>接口地址</dt><dd>{{modelInfo.values.MODEL_BASE_URL||'厂商默认地址'}}</dd><dt>配置来源</dt><dd>{{agent.is_default?'默认 Agent 配置':modelInfo.inherited?'继承实例配置':'Agent 独立配置'}}</dd><dt>API Key</dt><dd>{{modelInfo.hasKey?'已配置':'未配置'}}</dd></template><dt>角色定义</dt><dd>{{agent.role||'—'}}</dd><dt>状态</dt><dd>{{agent.online?'在线':'已停止'}}</dd><template v-if="remote"><dt>连接来源</dt><dd>{{agent.remote_address||'地址未知'}}<small class="binding-note">（IP:端口，非远端 Web 地址）</small></dd><dt>所属节点</dt><dd>{{agent.node_name}} · {{agent.node_id}}</dd></template></dl></div>
   <div class="row-actions"><el-button type="primary" native-type="button" :disabled="busy||(remote&&!agent.online)" @click="testing=true">测试聊天</el-button><template v-if="!remote"><el-button type="default" native-type="button" v-if="agent.is_default" class="secondary" :disabled="busy" @click="edit(agent)">编辑角色</el-button><template v-if="!agent.is_default"><el-button type="default" native-type="button" v-if="agent.kind==='virtual'" class="secondary" @click="editVirtual(agent)">编辑协作配置</el-button><el-button type="default" native-type="button" v-else class="secondary" :disabled="agent.online||busy" @click="edit(agent)">编辑</el-button><el-button type="default" native-type="button" class="quiet-button destructive" :disabled="busy||(agent.kind==='local'&&agent.online)" @click="deleting=true">删除</el-button></template><el-button type="default" native-type="button" v-if="agent.kind==='local'&&!agent.is_default" class="secondary" :disabled="busy" @click="agent.online?stop():start()">{{agent.online?'停止…':'启动'}}</el-button></template></div>
   <p v-if="agent.kind==='local'&&agent.online&&!agent.is_default" class="binding-note">修改或删除前请先停止 Agent，避免中断正在执行的任务。</p>
   <AdminConfiguration v-if="agent.is_default" :request="request" expanded @saved="()=>{perform(load);loadConfiguration()}"/>
   <el-alert type="warning" :closable="false" show-icon v-if="deleting" class="approval"><h4>删除 {{agent.name||agent.id}}？</h4><p>移除配置，历史聊天记录保留。仍被项目或虚拟 Agent 引用时不能删除。</p><el-button type="primary" native-type="button" class="destructive" :disabled="busy" @click="remove">确认删除</el-button><el-button type="default" native-type="button" class="secondary" @click="deleting=false">取消</el-button></el-alert>
  </template>

  <p v-if="selectedAgent&&!agent&&!busy" class="binding-note">此 Agent 不可用或远端已离线。刷新列表后重试。</p>
  <el-alert type="warning" :closable="false" show-icon v-if="pending" class="approval"><h4>需要确认</h4><p>{{pending.warning||'停用后不再接收新任务，已开始的任务继续执行。'}}</p><p>{{pending.input?.url}}</p><el-button type="primary" native-type="button" :disabled="busy" @click="decide(true)">确认执行</el-button><el-button type="default" native-type="button" class="secondary" :disabled="busy" @click="decide(false)">拒绝</el-button></el-alert>
 </div>
</ConfigPanel>
<el-drawer v-model="editingOpen" :before-close="closeEditor" direction="rtl" size="min(900px, 100vw)" :title="agent?.is_default?'编辑角色':selectedAgent?'编辑 Agent':'新建 Agent'" append-to-body destroy-on-close>
 <el-alert v-if="error" type="error" :closable="false" :title="error"/>
  <label v-if="!selectedAgent" class="field-stack">Agent 类型<el-radio-group v-model="creationKind" @change="chooseCreation"><el-radio-button value="local">本地 Agent</el-radio-button><el-radio-button value="virtual">虚拟 Agent</el-radio-button></el-radio-group></label>
  <GroupConfiguration v-if="virtual&&!selectedAgent" :group="virtual" :project="project" :request="request" creating virtual-agent inline @close="virtual=null" @saved="row=>{virtual=null;perform(load);$emit('select',row.key)}"/>
  <el-form v-if="draft" class="form-card" @submit.prevent="save" label-position="top"><div class="section-heading"><h3>{{selectedAgent?'编辑 Agent':'新建本地 Agent'}}</h3><p>选择运行器，定义这个 Agent 的身份与职责。</p></div><div class="form-grid"><label>Agent ID<el-input v-model="draft.client_id" :readonly="!!selectedAgent" required pattern="[a-zA-Z0-9_-]{1,64}" aria-label="Agent ID" /></label><label>运行器<el-select v-model="draft.provider" :disabled="agent?.is_default" aria-label="Agent 运行器"><el-option :label="&quot;crabot&quot;" :value="&quot;crabot&quot;" /><el-option :label="&quot;codex&quot;" :value="&quot;codex&quot;" /><el-option :label="&quot;claude&quot;" :value="&quot;claude&quot;" /><el-option :label="&quot;opencode&quot;" :value="&quot;opencode&quot;" /><el-option :label="&quot;mock&quot;" :value="&quot;mock&quot;" /></el-select></label><label class="agent-role">角色定义<el-input type="textarea" v-model="draft.role" required maxlength="255" rows="3" aria-label="Agent 默认角色" placeholder="例如：负责代码实现、单元测试与问题修复"></el-input><small>添加到项目时自动填入成员职责，项目可以独立覆盖。</small></label><label class="agent-role">回复要求（提示词）<el-input type="textarea" v-model="draft.response_instructions" :rows="3" maxlength="8192" aria-label="Agent 回复要求"/><small>随每次任务提供给此 Agent；默认只回答关键信息。</small></label></div><div v-if="!agent?.is_default" class="agent-runtime-editor"><el-checkbox v-if="draft.provider==='crabot'" v-model="useInstance">继承实例配置</el-checkbox><p v-if="inheritsInstance" class="binding-note">取消勾选可配置此 Agent 专属的模型与环境变量。</p><RuntimeConfigurationFields v-else v-model:provider="draft.provider" :show-provider="false" :values="draft.configuration" v-model:secret="modelKey" v-model:clear-secret="clearModelKey" :has-key="agent?.configuration?.has_api_key" :request="props.request"/></div><p class="binding-note">保存不会自动执行任务。独立配置仅影响此 Agent；Codex / Claude / OpenCode 仍需本机安装并完成认证。</p><div class="editor-actions"><el-button type="default" native-type="button" v-if="selectedAgent" class="secondary" @click="draft=null">取消</el-button><el-button native-type="button" :disabled="busy" @click="save(true)">保存并测试</el-button><el-button type="primary" native-type="submit" :disabled="busy">保存 Agent</el-button></div></el-form>
</el-drawer>
<el-drawer v-model="testing" direction="rtl" size="min(680px, 100vw)" :title="'测试聊天 · '+(agent?.name||agent?.id||'Agent')" :with-header="false" append-to-body destroy-on-close class="agent-test-drawer">
 <AgentTestChat v-if="testing&&agent" :key="agent.id" :agent="agent" :project="project" :request="request" :stream="stream" @close="testing=false"/>
</el-drawer>
</template>
<style scoped>
.agent-runtime-editor{margin:24px 0;padding-top:20px;border-top:1px solid var(--line)}.agent-runtime-editor>.el-checkbox{margin-bottom:16px}.agent-detail{overflow:auto;display:flex;flex-direction:column;gap:20px;padding:28px clamp(20px,5vw,72px)}
.agent-detail>*{flex-shrink:0;width:100%;max-width:960px;margin-left:auto;margin-right:auto;box-sizing:border-box}
.agent-overview{border:1px solid var(--line);background:var(--panel);border-radius:16px;overflow:hidden}
.agent-identity{display:flex;align-items:center;gap:18px;padding:24px;border-bottom:1px solid var(--line);background:linear-gradient(110deg,var(--raised),var(--panel))}
.agent-identity h3{font-size:22px;margin:6px 0 10px}.agent-monogram{display:grid;place-items:center;width:64px;height:64px;border-radius:18px;background:var(--panel);color:var(--accent);font-size:28px;border:1px solid var(--line)}
.agent-facts{display:grid;grid-template-columns:100px minmax(0,1fr);gap:14px;margin:0;padding:22px 24px;font-size:13px}
.agent-facts dt{color:var(--muted)}.agent-facts dd{margin:0;overflow-wrap:anywhere}
.row-actions{display:flex;flex-wrap:wrap;gap:10px}.agent-role{grid-column:1/-1}
.form-card{padding:24px;background:var(--panel);border:1px solid var(--line);border-radius:16px}
.form-grid{gap:22px}.form-grid label{display:grid;gap:9px}.form-grid input,.form-grid select{width:100%;box-sizing:border-box}
.binding-note{margin-top:0;line-height:1.7}.editor-actions{padding-top:20px}
</style>
