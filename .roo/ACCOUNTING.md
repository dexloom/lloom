Accounting scheme is following : 

executor provide model and price for inbound and outboud tokens. This price is big decimal number. alloy UINT256 should be used. 

Sending request client signs hash of request along with those prices and also address of executor. We also need nonce field. 

I want thiose parameters were fields LlmRequest and LlmResponse structures. 

Executor verifies signature process request, counts inbound and oputboud tokens signs this information and sends to smart contract. 



Smart contract needs function processRequest and proicessRequestSigned that implements eip712 logic.

processRequest verifies client signature with eip-712 and uses msg.sender to identify executor, 

processRequestSigned verifies both signatures. 

Please develop and implement eip-712 scheme. 

I want you to move blockchain part to core or other place and implement following logic. 





Blockcahin will be used for all three types of clients: executor, validator and client. 
