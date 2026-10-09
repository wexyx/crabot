import template from '../../../conf/response.md?raw'

// Keep newly created Agent defaults identical to the server's inherited template.
export const defaultResponse = template.trimEnd()
