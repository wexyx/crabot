import test from 'node:test'
import assert from 'node:assert/strict'
import {exampleArguments} from './tool-examples.js'
import {attachmentMessage,splitAttachments,validateAttachment,attachmentUrl} from './attachments.js'
test('tool examples preserve defaults, enum values and nested fields',()=>{
 assert.deepEqual(exampleArguments({type:'object',properties:{count:{type:'integer',default:0},enabled:{type:'boolean',default:true},mode:{enum:['read','write']},args:{type:'array'},path:{type:'string'},nested:{type:'object',properties:{command:{type:'string'}}}}},'example'),{count:0,enabled:true,mode:'read',args:[],path:'./README.md',nested:{command:'pwd'}})
})
test('attachment links are instance-local and input limits are enforced',()=>{
 const ref='[附件：test.png](/v1/attachments/11111111-1111-4111-8111-111111111111/content)'
 const body=attachmentMessage('',[{reference:ref}]);assert.ok(body.startsWith('请分析'));assert.equal(splitAttachments(body).files.length,1)
 assert.equal(splitAttachments('[附件：evil](https://example.com/image)').files.length,0)
 assert.throws(()=>validateAttachment({size:11*1024*1024,type:'text/plain'},0));assert.throws(()=>validateAttachment({size:1,type:'image/png'},8))
})

test('image links follow the selected Agent rather than the Web origin',()=>{
 const path='/v1/attachments/11111111-1111-4111-8111-111111111111/content'
 assert.equal(attachmentUrl(path,'https://crabot.example'), 'https://crabot.example'+path)
 assert.throws(()=>attachmentUrl('http://169.254.169.254/','https://crabot.example'))
 assert.throws(()=>attachmentUrl('/v1/attachments/../../etc/passwd','https://crabot.example'))
})
