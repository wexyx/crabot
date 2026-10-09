<script setup>
import {computed,ref,watch} from 'vue'
import AgentEnvironmentFields from './AgentEnvironmentFields.vue'
import {modelLabel,readCatalog} from './opencode-models.js'
const props=defineProps({values:{type:Object,required:true},hasKey:Boolean,showProvider:{type:Boolean,default:true},request:{type:Function,default:null}})
const provider=defineModel('provider',{type:String})
const secret=defineModel('secret',{type:String,default:''})
const clearSecret=defineModel('clearSecret',{type:Boolean,default:false})
const models=ref([]),modelNote=ref(''),modelError=ref(''),loadingModels=ref(false)
const choices=computed(()=>models.value.map(model=>({value:model.id,label:modelLabel(model),free:model.free===true})))
const freeCount=computed(()=>models.value.filter(model=>model.free).length)
// Discovery is host introspection, so it runs on demand and never blocks editing.
async function loadModels(){
 if(!props.request||loadingModels.value)return
 loadingModels.value=true;modelError.value=''
 // A per-Agent override can name its own launcher, so ask about that one.
 const bin=String(props.values.OPENCODE_BIN||'').trim()
 const query=bin?`?bin=${encodeURIComponent(bin)}`:''
 try{const catalog=readCatalog(await props.request('/v1/opencode/models'+query));models.value=catalog.models;modelNote.value=catalog.reason||(catalog.priced?`${catalog.models.length} 个可用模型，其中 ${freeCount.value} 个免费`:'仅能列出模型 ID，价格未知')}
 catch(e){models.value=[];modelNote.value='';modelError.value=e.message}
 finally{loadingModels.value=false}
}
watch(provider,value=>{if(value==='opencode')loadModels()},{immediate:true})
// Editing the launcher changes which CLI answers, so the list must follow it.
watch(()=>props.values.OPENCODE_BIN,()=>{if(provider.value==='opencode')loadModels()})
</script>
<template>
 <div class="runtime-fields">
  <el-form-item v-if="showProvider" label="运行器"><el-select v-model="provider" aria-label="配置运行器"><el-option v-for="value in ['crabot','codex','claude','opencode','mock']" :key="value" :value="value" :label="value"/></el-select></el-form-item>
  <template v-if="provider==='crabot'">
   <el-form-item label="模型厂商"><el-select v-model="values.MODEL_PROVIDER" aria-label="模型厂商"><el-option v-for="value in ['openai','anthropic','gemini','deepseek','qwen','ark','ollama','compatible']" :key="value" :value="value" :label="value"/></el-select></el-form-item>
   <el-form-item label="上下文长度"><el-input v-model="values.CONTEXT_MAX_TOKENS" placeholder="65536（含输出预留）" aria-label="上下文长度"/></el-form-item>
   <el-form-item label="最大输出长度"><el-input v-model="values.HARNESS_MAX_TOKENS" placeholder="4096" aria-label="最大输出长度"/><small>单次请求的输出上限，不是目标字数。</small></el-form-item>
   <template v-if="values.MODEL_PROVIDER==='deepseek' &amp;&amp; values.MODEL_API!=='anthropic'">
    <el-form-item label="思考模式"><el-select :model-value="values.MODEL_THINKING||'disabled'" @update:model-value="values.MODEL_THINKING=$event" aria-label="思考模式"><el-option value="disabled" label="关闭 · 优先速度"/><el-option value="enabled" label="开启 · 复杂任务"/></el-select></el-form-item>
    <el-form-item v-if="values.MODEL_THINKING==='enabled'" label="思考强度"><el-select :model-value="values.MODEL_REASONING_EFFORT||'low'" @update:model-value="values.MODEL_REASONING_EFFORT=$event" aria-label="思考强度"><el-option value="low" label="低 · 更快"/><el-option value="high" label="高"/><el-option value="max" label="最高 · 更慢"/></el-select></el-form-item>
   </template>
   <el-form-item label="模型名称"><el-input v-model="values.MODEL_NAME" required placeholder="例如 deepseek-chat" aria-label="模型名称"/></el-form-item>
    <el-form-item label="接口地址"><el-input v-model="values.MODEL_BASE_URL" placeholder="留空使用厂商默认地址" aria-label="接口地址"/></el-form-item>
    <el-form-item label="接口协议"><el-select v-model="values.MODEL_API" aria-label="接口协议"><el-option value="" label="厂商默认"/><el-option v-for="value in ['chat','responses','anthropic']" :key="value" :value="value" :label="value"/></el-select></el-form-item>
    <el-form-item class="system-prompt" label="系统提示词"><el-input v-model="values.MODEL_SYSTEM_PROMPT" type="textarea" :rows="5" placeholder="留空使用内置默认提示词" aria-label="系统提示词"/><small>发送给模型的全局行为规则；清空后恢复内置默认。</small></el-form-item>
    <el-form-item label="API Key"><el-input v-model="secret" type="password" autocomplete="new-password" :placeholder="hasKey?'已设置，留空保留':'填写密钥（不回显已保存内容）'" aria-label="模型 API Key"/></el-form-item>
   <el-form-item label="密钥操作"><el-checkbox v-model="clearSecret">清除已有 API Key</el-checkbox></el-form-item>
  </template>
  <el-form-item v-else-if="provider==='codex'" label="Codex 启动命令"><el-input v-model="values.CODEX_BIN" placeholder="codex 或 codex --model 模型名" aria-label="Codex 启动命令"/><small>支持命令名、完整路径及参数；含空格的路径请加引号。不需要填写 exec、输出格式或权限参数。</small></el-form-item>
  <el-form-item v-else-if="provider==='claude'" label="Claude 启动命令"><el-input v-model="values.CLAUDE_BIN" placeholder="claude 或 claude --model sonnet" aria-label="Claude 启动命令"/><small>支持命令名、完整路径及参数；含空格的路径请加引号。不需要填写 -p、输出格式或权限参数。</small></el-form-item>
  <template v-else-if="provider==='opencode'">
   <el-form-item label="OpenCode 启动命令"><el-input v-model="values.OPENCODE_BIN" placeholder="opencode" aria-label="OpenCode 启动命令"/><small>支持命令名、完整路径及参数；含空格的路径请加引号。不需要填写 run、输出格式或审批参数。</small></el-form-item>
   <el-form-item class="model-picker" label="模型"><el-select v-model="values.OPENCODE_MODEL" filterable allow-create default-first-option clearable :loading="loadingModels" placeholder="留空使用 OpenCode 的默认模型" aria-label="OpenCode 模型"><el-option v-for="choice in choices" :key="choice.value" :value="choice.value" :label="choice.label"><span class="model-option"><span class="model-id">{{choice.value}}</span><span class="model-price" :class="{free:choice.free}">{{choice.label.split(' · ').slice(1).join(' · ')}}</span></span></el-option></el-select><small>从本机 OpenCode 拉取；免费模型无需账号，付费模型需登录，凭据通过下方环境变量提供。也可直接填写 provider/model。</small><button type="button" class="model-refresh" @click="loadModels">重新拉取模型列表</button><small v-if="modelNote">{{modelNote}}</small><small v-if="modelError" role="alert">{{modelError}}</small></el-form-item>
   <el-form-item label="Agent 档案"><el-input v-model="values.OPENCODE_AGENT" placeholder="留空使用默认 Agent" aria-label="OpenCode Agent 档案"/></el-form-item>
   <el-form-item label="自动批准工具"><el-select :model-value="values.OPENCODE_AUTO_APPROVE||'false'" @update:model-value="values.OPENCODE_AUTO_APPROVE=$event" aria-label="自动批准工具"><el-option value="false" label="关闭 · 每次询问"/><el-option value="true" label="开启 · 自动批准未拒绝项"/></el-select><small>关闭时 OpenCode 每次工具调用都会询问；非交互运行无法应答。</small></el-form-item>
   <el-form-item label="思考模式"><el-select :model-value="values.OPENCODE_THINKING??'true'" @update:model-value="values.OPENCODE_THINKING=$event" aria-label="OpenCode 思考模式"><el-option value="false" label="关闭 · 更快"/><el-option value="true" label="开启 · 推理模型更准"/></el-select><small>关闭可减少每个请求的等待时间；非推理模型通常没有区别。</small></el-form-item>
   <el-form-item label="独立服务"><el-select :model-value="values.OPENCODE_STANDALONE??'true'" @update:model-value="values.OPENCODE_STANDALONE=$event" aria-label="OpenCode 独立服务"><el-option value="true" label="开启 · 每轮自启服务，默认"/><el-option value="false" label="关闭 · 复用后台服务，更快"/></el-select><small>关闭后需能找到本机 OpenCode 后台服务；Crabot 默认不共享你的 HOME，此时关闭会等待超时。</small></el-form-item>
  </template>
  <p v-else class="runtime-note">Mock 仅验证通信链路，不调用模型。</p>
  <AgentEnvironmentFields v-if="provider!=='mock'" :values="values"/>
 </div>
</template>
<style scoped>
.runtime-fields{min-width:0;width:100%;box-sizing:border-box;display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:18px 24px;grid-column:1/-1}
.el-form-item{margin:0;min-width:0}.runtime-note{font-size:12px;color:var(--muted);align-self:center}
.model-picker,.system-prompt{grid-column:1/-1}
.model-picker small,.system-prompt small{display:block;margin-top:6px;line-height:1.6;color:var(--muted)}
.model-refresh{margin-top:8px;padding:0;border:0;background:none;color:var(--accent);font-size:12px;cursor:pointer;text-decoration:underline}
.model-option{display:flex;gap:12px;align-items:baseline;justify-content:space-between;width:100%}
.model-id{font-weight:600}
.model-price{color:var(--muted);font-size:12px;white-space:nowrap}
.model-price.free{color:var(--success,#5fd08a);font-weight:600}
@media(max-width:650px){.runtime-fields{grid-template-columns:1fr}.model-option{flex-direction:column;gap:2px}}
</style>
