// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test, console2} from "forge-std/Test.sol";
import {Vm} from "forge-std/Vm.sol";
import {AccountingV2} from "../src/Accounting.sol";

contract AccountingV2Test is Test {
    AccountingV2 public accounting;
    
    address public client = address(0x1);
    address public executor = address(0x2);
    address public wrongClient = address(0x3);
    address public wrongExecutor = address(0x4);
    
    // Test keys for signing (use vm.sign with these)
    uint256 internal clientPrivateKey = 0x1;
    uint256 internal executorPrivateKey = 0x2;
    uint256 internal wrongClientPrivateKey = 0x3;
    uint256 internal wrongExecutorPrivateKey = 0x4;
    
    function setUp() public {
        accounting = new AccountingV2();
        
        // Ensure test addresses match private keys
        client = vm.addr(clientPrivateKey);
        executor = vm.addr(executorPrivateKey);
        wrongClient = vm.addr(wrongClientPrivateKey);
        wrongExecutor = vm.addr(wrongExecutorPrivateKey);
    }
    
    // =============================================================================
    // Helper Functions
    // =============================================================================
    
    function createValidRequest() internal view returns (AccountingV2.LlmRequestCommitment memory) {
        return AccountingV2.LlmRequestCommitment({
            executor: executor,
            model: "gpt-4",
            promptHash: keccak256("test prompt"),
            systemPromptHash: keccak256("test system prompt"),
            maxTokens: 1000,
            temperature: 7000, // 0.7 * 10000
            inboundPrice: 1000000000000000, // 0.001 ETH per token
            outboundPrice: 2000000000000000, // 0.002 ETH per token
            nonce: 1,
            deadline: uint64(block.timestamp + 3600)
        });
    }
    
    function createValidResponse(bytes32 requestHash) internal view returns (AccountingV2.LlmResponseCommitment memory) {
        return AccountingV2.LlmResponseCommitment({
            requestHash: requestHash,
            client: client,
            model: "gpt-4",
            contentHash: keccak256("test response"),
            inboundTokens: 100,
            outboundTokens: 200,
            inboundPrice: 1000000000000000,
            outboundPrice: 2000000000000000,
            timestamp: uint64(block.timestamp),
            success: true
        });
    }
    
    function signRequest(AccountingV2.LlmRequestCommitment memory request, uint256 privateKey) 
        internal view returns (bytes memory) {
        bytes32 messageHash = accounting.getRequestMessageHash(request);
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(privateKey, messageHash);
        return abi.encodePacked(r, s, v);
    }
    
    function signResponse(AccountingV2.LlmResponseCommitment memory response, uint256 privateKey) 
        internal view returns (bytes memory) {
        bytes32 messageHash = accounting.getResponseMessageHash(response);
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(privateKey, messageHash);
        return abi.encodePacked(r, s, v);
    }
    
    function testDomainSeparator() public view {
        bytes32 domainSeparator = accounting.DOMAIN_SEPARATOR();
        assertFalse(domainSeparator == bytes32(0), "Domain separator should not be zero");
    }
    
    function testContractDeployment() public view {
        assertEq(accounting.owner(), address(this), "Owner should be deployer");
        assertEq(accounting.DOMAIN_NAME(), "Lloom Network", "Domain name mismatch");
        assertEq(accounting.DOMAIN_VERSION(), "1.0.0", "Domain version mismatch");
    }
    
    function testNonceManagement() public view {
        uint64 initialNonce = accounting.getCurrentNonce(client);
        assertEq(initialNonce, 0, "Initial nonce should be 0");
    }
    
    function testNetworkStats() public view {
        (uint64 totalInbound, uint64 totalOutbound, uint256 totalVol, uint32 totalReq) = accounting.getNetworkStats();
        assertEq(totalInbound, 0, "Initial inbound tokens should be 0");
        assertEq(totalOutbound, 0, "Initial outbound tokens should be 0");
        assertEq(totalVol, 0, "Initial volume should be 0");
        assertEq(totalReq, 0, "Initial requests should be 0");
    }
    
    function testExecutorStats() public view {
        AccountingV2.ExecutorStats memory stats = accounting.getExecutorStats(executor);
        assertEq(stats.totalInboundTokens, 0, "Initial executor inbound tokens should be 0");
        assertEq(stats.totalOutboundTokens, 0, "Initial executor outbound tokens should be 0");
        assertEq(stats.totalRevenue, 0, "Initial executor revenue should be 0");
        assertEq(stats.requestCount, 0, "Initial executor request count should be 0");
        assertEq(stats.successfulRequests, 0, "Initial executor successful requests should be 0");
    }
    
    function testClientStats() public view {
        AccountingV2.ClientStats memory stats = accounting.getClientStats(client);
        assertEq(stats.totalInboundTokens, 0, "Initial client inbound tokens should be 0");
        assertEq(stats.totalOutboundTokens, 0, "Initial client outbound tokens should be 0");
        assertEq(stats.totalSpent, 0, "Initial client spent should be 0");
        assertEq(stats.requestCount, 0, "Initial client request count should be 0");
    }
    
    function testTypeHashes() public view {
        bytes32 requestTypeHash = accounting.LLMREQUEST_TYPEHASH();
        bytes32 responseTypeHash = accounting.LLMRESPONSE_TYPEHASH();
        bytes32 domainTypeHash = accounting.DOMAIN_TYPEHASH();
        
        // Verify the hashes are not zero
        assertFalse(requestTypeHash == bytes32(0), "Request type hash should not be zero");
        assertFalse(responseTypeHash == bytes32(0), "Response type hash should not be zero");
        assertFalse(domainTypeHash == bytes32(0), "Domain type hash should not be zero");
        
        // Verify expected values match specification
        bytes32 expectedRequestHash = keccak256(
            "LlmRequestCommitment(address executor,string model,bytes32 promptHash,bytes32 systemPromptHash,uint32 maxTokens,uint32 temperature,uint256 inboundPrice,uint256 outboundPrice,uint64 nonce,uint64 deadline)"
        );
        bytes32 expectedResponseHash = keccak256(
            "LlmResponseCommitment(bytes32 requestHash,address client,string model,bytes32 contentHash,uint32 inboundTokens,uint32 outboundTokens,uint256 inboundPrice,uint256 outboundPrice,uint64 timestamp,bool success)"
        );
        bytes32 expectedDomainHash = keccak256(
            "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)"
        );
        
        assertEq(requestTypeHash, expectedRequestHash, "Request type hash mismatch");
        assertEq(responseTypeHash, expectedResponseHash, "Response type hash mismatch");
        assertEq(domainTypeHash, expectedDomainHash, "Domain type hash mismatch");
    }
    
    function testOwnershipTransfer() public {
        address newOwner = address(0x3);
        accounting.transferOwnership(newOwner);
        assertEq(accounting.owner(), newOwner, "Ownership transfer failed");
    }
    
    function testGetRequestMessageHash() public view {
        AccountingV2.LlmRequestCommitment memory request = AccountingV2.LlmRequestCommitment({
            executor: executor,
            model: "gpt-4",
            promptHash: keccak256("test prompt"),
            systemPromptHash: keccak256("test system prompt"),
            maxTokens: 1000,
            temperature: 7000, // 0.7 * 10000
            inboundPrice: 1000000000000000, // 0.001 ETH per token
            outboundPrice: 2000000000000000, // 0.002 ETH per token
            nonce: 1,
            deadline: uint64(block.timestamp + 3600)
        });
        
        bytes32 messageHash = accounting.getRequestMessageHash(request);
        assertFalse(messageHash == bytes32(0), "Message hash should not be zero");
    }
    
    function testGetResponseMessageHash() public view {
        AccountingV2.LlmResponseCommitment memory response = AccountingV2.LlmResponseCommitment({
            requestHash: keccak256("test request"),
            client: client,
            model: "gpt-4",
            contentHash: keccak256("test response"),
            inboundTokens: 100,
            outboundTokens: 200,
            inboundPrice: 1000000000000000,
            outboundPrice: 2000000000000000,
            timestamp: uint64(block.timestamp),
            success: true
        });
        
        bytes32 messageHash = accounting.getResponseMessageHash(response);
        assertFalse(messageHash == bytes32(0), "Response message hash should not be zero");
    }
    
    // =============================================================================
    // processRequest Tests (Single Signature - Executor Only)
    // =============================================================================
    
    function testProcessRequestSuccess() public {
        AccountingV2.LlmRequestCommitment memory request = createValidRequest();
        bytes32 requestHash = accounting.getRequestMessageHash(request);
        AccountingV2.LlmResponseCommitment memory response = createValidResponse(requestHash);
        
        bytes memory clientSignature = signRequest(request, clientPrivateKey);
        
        vm.prank(executor);
        accounting.processRequest(
            request,
            response,
            clientSignature,
            "test prompt",
            "test system prompt",
            "test response"
        );
        
        // Verify nonce was incremented
        assertEq(accounting.getCurrentNonce(client), 1, "Nonce should be incremented");
        
        // Verify statistics
        AccountingV2.ExecutorStats memory execStats = accounting.getExecutorStats(executor);
        assertEq(execStats.totalInboundTokens, 100, "Executor inbound tokens mismatch");
        assertEq(execStats.totalOutboundTokens, 200, "Executor outbound tokens mismatch");
        assertEq(execStats.requestCount, 1, "Executor request count mismatch");
        assertEq(execStats.successfulRequests, 1, "Executor successful requests mismatch");
        
        AccountingV2.ClientStats memory clientStatsData = accounting.getClientStats(client);
        assertEq(clientStatsData.totalInboundTokens, 100, "Client inbound tokens mismatch");
        assertEq(clientStatsData.totalOutboundTokens, 200, "Client outbound tokens mismatch");
        assertEq(clientStatsData.requestCount, 1, "Client request count mismatch");
    }
    
    function testProcessRequestInvalidClientSignature() public {
        AccountingV2.LlmRequestCommitment memory request = createValidRequest();
        bytes32 requestHash = accounting.getRequestMessageHash(request);
        AccountingV2.LlmResponseCommitment memory response = createValidResponse(requestHash);
        
        // Sign with wrong private key
        bytes memory wrongSignature = signRequest(request, wrongClientPrivateKey);
        
        vm.prank(executor);
        vm.expectRevert("Client address mismatch");
        accounting.processRequest(
            request,
            response,
            wrongSignature,
            "test prompt",
            "test system prompt",
            "test response"
        );
    }
    
    function testProcessRequestCorrectNonce() public {
        AccountingV2.LlmRequestCommitment memory request = createValidRequest();
        request.nonce = 1; // Should be current nonce + 1
        bytes32 requestHash = accounting.getRequestMessageHash(request);
        AccountingV2.LlmResponseCommitment memory response = createValidResponse(requestHash);
        
        bytes memory clientSignature = signRequest(request, clientPrivateKey);
        
        vm.prank(executor);
        accounting.processRequest(
            request,
            response,
            clientSignature,
            "test prompt",
            "test system prompt",
            "test response"
        );
        
        assertEq(accounting.getCurrentNonce(client), 1, "Nonce should be 1 after processing");
    }
    
    function testProcessRequestWrongNonce() public {
        AccountingV2.LlmRequestCommitment memory request = createValidRequest();
        request.nonce = 5; // Wrong nonce (should be 1)
        bytes32 requestHash = accounting.getRequestMessageHash(request);
        AccountingV2.LlmResponseCommitment memory response = createValidResponse(requestHash);
        
        bytes memory clientSignature = signRequest(request, clientPrivateKey);
        
        vm.prank(executor);
        vm.expectRevert("Invalid nonce");
        accounting.processRequest(
            request,
            response,
            clientSignature,
            "test prompt",
            "test system prompt",
            "test response"
        );
    }
    
    function testProcessRequestReplayAttack() public {
        AccountingV2.LlmRequestCommitment memory request = createValidRequest();
        bytes32 requestHash = accounting.getRequestMessageHash(request);
        AccountingV2.LlmResponseCommitment memory response = createValidResponse(requestHash);
        
        bytes memory clientSignature = signRequest(request, clientPrivateKey);
        
        // First request should succeed
        vm.prank(executor);
        accounting.processRequest(
            request,
            response,
            clientSignature,
            "test prompt",
            "test system prompt",
            "test response"
        );
        
        // Second request with same nonce should fail
        vm.prank(executor);
        vm.expectRevert("Invalid nonce");
        accounting.processRequest(
            request,
            response,
            clientSignature,
            "test prompt",
            "test system prompt",
            "test response"
        );
    }
    
    function testProcessRequestExpiredDeadline() public {
        AccountingV2.LlmRequestCommitment memory request = createValidRequest();
        request.deadline = uint64(block.timestamp - 1); // Expired deadline
        bytes32 requestHash = accounting.getRequestMessageHash(request);
        AccountingV2.LlmResponseCommitment memory response = createValidResponse(requestHash);
        
        bytes memory clientSignature = signRequest(request, clientPrivateKey);
        
        vm.prank(executor);
        vm.expectRevert("Request deadline has passed");
        accounting.processRequest(
            request,
            response,
            clientSignature,
            "test prompt",
            "test system prompt",
            "test response"
        );
    }
    
    function testProcessRequestInboundPriceMismatch() public {
        AccountingV2.LlmRequestCommitment memory request = createValidRequest();
        bytes32 requestHash = accounting.getRequestMessageHash(request);
        AccountingV2.LlmResponseCommitment memory response = createValidResponse(requestHash);
        response.inboundPrice = 9999999999999999; // Different price
        
        bytes memory clientSignature = signRequest(request, clientPrivateKey);
        
        vm.prank(executor);
        vm.expectRevert("Inbound price mismatch");
        accounting.processRequest(
            request,
            response,
            clientSignature,
            "test prompt",
            "test system prompt",
            "test response"
        );
    }
    
    function testProcessRequestOutboundPriceMismatch() public {
        AccountingV2.LlmRequestCommitment memory request = createValidRequest();
        bytes32 requestHash = accounting.getRequestMessageHash(request);
        AccountingV2.LlmResponseCommitment memory response = createValidResponse(requestHash);
        response.outboundPrice = 9999999999999999; // Different price
        
        bytes memory clientSignature = signRequest(request, clientPrivateKey);
        
        vm.prank(executor);
        vm.expectRevert("Outbound price mismatch");
        accounting.processRequest(
            request,
            response,
            clientSignature,
            "test prompt",
            "test system prompt",
            "test response"
        );
    }
    
    function testProcessRequestPromptHashMismatch() public {
        AccountingV2.LlmRequestCommitment memory request = createValidRequest();
        bytes32 requestHash = accounting.getRequestMessageHash(request);
        AccountingV2.LlmResponseCommitment memory response = createValidResponse(requestHash);
        
        bytes memory clientSignature = signRequest(request, clientPrivateKey);
        
        vm.prank(executor);
        vm.expectRevert("Prompt hash mismatch");
        accounting.processRequest(
            request,
            response,
            clientSignature,
            "different prompt", // Wrong prompt content
            "test system prompt",
            "test response"
        );
    }
    
    function testProcessRequestSystemPromptHashMismatch() public {
        AccountingV2.LlmRequestCommitment memory request = createValidRequest();
        bytes32 requestHash = accounting.getRequestMessageHash(request);
        AccountingV2.LlmResponseCommitment memory response = createValidResponse(requestHash);
        
        bytes memory clientSignature = signRequest(request, clientPrivateKey);
        
        vm.prank(executor);
        vm.expectRevert("System prompt hash mismatch");
        accounting.processRequest(
            request,
            response,
            clientSignature,
            "test prompt",
            "different system prompt", // Wrong system prompt content
            "test response"
        );
    }
    
    function testProcessRequestResponseHashMismatch() public {
        AccountingV2.LlmRequestCommitment memory request = createValidRequest();
        bytes32 requestHash = accounting.getRequestMessageHash(request);
        AccountingV2.LlmResponseCommitment memory response = createValidResponse(requestHash);
        
        bytes memory clientSignature = signRequest(request, clientPrivateKey);
        
        vm.prank(executor);
        vm.expectRevert("Response content hash mismatch");
        accounting.processRequest(
            request,
            response,
            clientSignature,
            "test prompt",
            "test system prompt",
            "different response" // Wrong response content
        );
    }
    
    function testProcessRequestStatisticsUpdate() public {
        AccountingV2.LlmRequestCommitment memory request = createValidRequest();
        bytes32 requestHash = accounting.getRequestMessageHash(request);
        AccountingV2.LlmResponseCommitment memory response = createValidResponse(requestHash);
        
        bytes memory clientSignature = signRequest(request, clientPrivateKey);
        
        vm.prank(executor);
        accounting.processRequest(
            request,
            response,
            clientSignature,
            "test prompt",
            "test system prompt",
            "test response"
        );
        
        // Check network stats
        (uint64 totalInbound, uint64 totalOutbound, uint256 totalVol, uint32 totalReq) = accounting.getNetworkStats();
        assertEq(totalInbound, 100, "Network inbound tokens mismatch");
        assertEq(totalOutbound, 200, "Network outbound tokens mismatch");
        assertEq(totalReq, 1, "Network total requests mismatch");
        
        uint256 expectedCost = (100 * 1000000000000000) + (200 * 2000000000000000);
        assertEq(totalVol, expectedCost, "Network total volume mismatch");
    }
    
    function testProcessRequestEventEmission() public {
        AccountingV2.LlmRequestCommitment memory request = createValidRequest();
        bytes32 requestHash = accounting.getRequestMessageHash(request);
        AccountingV2.LlmResponseCommitment memory response = createValidResponse(requestHash);
        
        bytes memory clientSignature = signRequest(request, clientPrivateKey);
        
        uint256 expectedCost = (100 * 1000000000000000) + (200 * 2000000000000000);
        
        // Test that events are emitted by checking for any events
        vm.recordLogs();
        
        vm.prank(executor);
        accounting.processRequest(
            request,
            response,
            clientSignature,
            "test prompt",
            "test system prompt",
            "test response"
        );
        
        // Verify that logs were emitted (events were triggered)
        Vm.Log[] memory logs = vm.getRecordedLogs();
        assertTrue(logs.length >= 3, "Should emit at least 3 events");
        
        // Verify the first event (NonceIncremented) has the correct topic
        assertEq(logs[0].topics[0], keccak256("NonceIncremented(address,uint64)"), "First event should be NonceIncremented");
        
        // Verify the second event (RequestProcessed) has the correct topic
        assertEq(logs[1].topics[0], keccak256("RequestProcessed(bytes32,address,address,string,uint32,uint32,uint256,bool)"), "Second event should be RequestProcessed");
        
        // Verify the third event (PriceCommitment) has the correct topic
        assertEq(logs[2].topics[0], keccak256("PriceCommitment(address,address,string,uint256,uint256)"), "Third event should be PriceCommitment");
    }
    
    function testProcessRequestExecutorMismatch() public {
        AccountingV2.LlmRequestCommitment memory request = createValidRequest();
        bytes32 requestHash = accounting.getRequestMessageHash(request);
        AccountingV2.LlmResponseCommitment memory response = createValidResponse(requestHash);
        
        bytes memory clientSignature = signRequest(request, clientPrivateKey);
        
        vm.prank(wrongExecutor); // Wrong executor calling
        vm.expectRevert("Executor mismatch");
        accounting.processRequest(
            request,
            response,
            clientSignature,
            "test prompt",
            "test system prompt",
            "test response"
        );
    }
    
    // =============================================================================
    // processRequestSigned Tests (Dual Signature)
    // =============================================================================
    
    function testProcessRequestSignedSuccess() public {
        AccountingV2.LlmRequestCommitment memory request = createValidRequest();
        bytes32 requestHash = accounting.getRequestMessageHash(request);
        AccountingV2.LlmResponseCommitment memory response = createValidResponse(requestHash);
        
        bytes memory clientSignature = signRequest(request, clientPrivateKey);
        bytes memory executorSignature = signResponse(response, executorPrivateKey);
        
        accounting.processRequestSigned(
            request,
            response,
            clientSignature,
            executorSignature
        );
        
        // Verify nonce was incremented
        assertEq(accounting.getCurrentNonce(client), 1, "Nonce should be incremented");
        
        // Verify statistics
        AccountingV2.ExecutorStats memory execStats = accounting.getExecutorStats(executor);
        assertEq(execStats.totalInboundTokens, 100, "Executor inbound tokens mismatch");
        assertEq(execStats.totalOutboundTokens, 200, "Executor outbound tokens mismatch");
        assertEq(execStats.requestCount, 1, "Executor request count mismatch");
        assertEq(execStats.successfulRequests, 1, "Executor successful requests mismatch");
    }
    
    function testProcessRequestSignedInvalidClientSignature() public {
        AccountingV2.LlmRequestCommitment memory request = createValidRequest();
        bytes32 requestHash = accounting.getRequestMessageHash(request);
        AccountingV2.LlmResponseCommitment memory response = createValidResponse(requestHash);
        
        bytes memory wrongClientSignature = signRequest(request, wrongClientPrivateKey);
        bytes memory executorSignature = signResponse(response, executorPrivateKey);
        
        vm.expectRevert("Client address mismatch");
        accounting.processRequestSigned(
            request,
            response,
            wrongClientSignature,
            executorSignature
        );
    }
    
    function testProcessRequestSignedInvalidExecutorSignature() public {
        AccountingV2.LlmRequestCommitment memory request = createValidRequest();
        bytes32 requestHash = accounting.getRequestMessageHash(request);
        AccountingV2.LlmResponseCommitment memory response = createValidResponse(requestHash);
        
        bytes memory clientSignature = signRequest(request, clientPrivateKey);
        bytes memory wrongExecutorSignature = signResponse(response, wrongExecutorPrivateKey);
        
        vm.expectRevert("Executor mismatch");
        accounting.processRequestSigned(
            request,
            response,
            clientSignature,
            wrongExecutorSignature
        );
    }
    
    function testProcessRequestSignedMismatchedRequestHash() public {
        AccountingV2.LlmRequestCommitment memory request = createValidRequest();
        AccountingV2.LlmResponseCommitment memory response = createValidResponse(keccak256("wrong hash"));
        
        bytes memory clientSignature = signRequest(request, clientPrivateKey);
        bytes memory executorSignature = signResponse(response, executorPrivateKey);
        
        vm.expectRevert("Request hash mismatch");
        accounting.processRequestSigned(
            request,
            response,
            clientSignature,
            executorSignature
        );
    }
    
    function testProcessRequestSignedPriceConsistency() public {
        AccountingV2.LlmRequestCommitment memory request = createValidRequest();
        bytes32 requestHash = accounting.getRequestMessageHash(request);
        AccountingV2.LlmResponseCommitment memory response = createValidResponse(requestHash);
        response.inboundPrice = 9999999999999999; // Different price
        
        bytes memory clientSignature = signRequest(request, clientPrivateKey);
        bytes memory executorSignature = signResponse(response, executorPrivateKey);
        
        vm.expectRevert("Inbound price mismatch");
        accounting.processRequestSigned(
            request,
            response,
            clientSignature,
            executorSignature
        );
    }
    
    function testProcessRequestSignedClientAddressMismatch() public {
        AccountingV2.LlmRequestCommitment memory request = createValidRequest();
        bytes32 requestHash = accounting.getRequestMessageHash(request);
        AccountingV2.LlmResponseCommitment memory response = createValidResponse(requestHash);
        response.client = wrongClient; // Wrong client address
        
        bytes memory clientSignature = signRequest(request, clientPrivateKey);
        bytes memory executorSignature = signResponse(response, executorPrivateKey);
        
        vm.expectRevert("Client address mismatch");
        accounting.processRequestSigned(
            request,
            response,
            clientSignature,
            executorSignature
        );
    }
    
    function testProcessRequestSignedModelMismatch() public {
        AccountingV2.LlmRequestCommitment memory request = createValidRequest();
        bytes32 requestHash = accounting.getRequestMessageHash(request);
        AccountingV2.LlmResponseCommitment memory response = createValidResponse(requestHash);
        response.model = "gpt-3.5"; // Different model
        
        bytes memory clientSignature = signRequest(request, clientPrivateKey);
        bytes memory executorSignature = signResponse(response, executorPrivateKey);
        
        vm.expectRevert("Model mismatch");
        accounting.processRequestSigned(
            request,
            response,
            clientSignature,
            executorSignature
        );
    }
    
    function testProcessRequestSignedExecutorMismatch() public {
        AccountingV2.LlmRequestCommitment memory request = createValidRequest();
        request.executor = wrongExecutor; // Wrong executor in request
        bytes32 requestHash = accounting.getRequestMessageHash(request);
        AccountingV2.LlmResponseCommitment memory response = createValidResponse(requestHash);
        
        bytes memory clientSignature = signRequest(request, clientPrivateKey);
        bytes memory executorSignature = signResponse(response, executorPrivateKey);
        
        vm.expectRevert("Executor mismatch");
        accounting.processRequestSigned(
            request,
            response,
            clientSignature,
            executorSignature
        );
    }
    
    // =============================================================================
    // Edge Cases and Security Tests
    // =============================================================================
    
    function testZeroTokenAmounts() public {
        AccountingV2.LlmRequestCommitment memory request = createValidRequest();
        bytes32 requestHash = accounting.getRequestMessageHash(request);
        AccountingV2.LlmResponseCommitment memory response = createValidResponse(requestHash);
        response.inboundTokens = 0;
        response.outboundTokens = 0;
        
        bytes memory clientSignature = signRequest(request, clientPrivateKey);
        bytes memory executorSignature = signResponse(response, executorPrivateKey);
        
        // Should succeed with zero tokens
        accounting.processRequestSigned(
            request,
            response,
            clientSignature,
            executorSignature
        );
        
        AccountingV2.ExecutorStats memory execStats = accounting.getExecutorStats(executor);
        assertEq(execStats.totalInboundTokens, 0, "Should have 0 inbound tokens");
        assertEq(execStats.totalOutboundTokens, 0, "Should have 0 outbound tokens");
        assertEq(execStats.totalRevenue, 0, "Should have 0 revenue");
    }
    
    function testMaxValueStress() public {
        AccountingV2.LlmRequestCommitment memory request = createValidRequest();
        request.inboundPrice = type(uint256).max / 2; // Large but safe value
        request.outboundPrice = type(uint256).max / 2;
        bytes32 requestHash = accounting.getRequestMessageHash(request);
        AccountingV2.LlmResponseCommitment memory response = createValidResponse(requestHash);
        response.inboundPrice = request.inboundPrice;
        response.outboundPrice = request.outboundPrice;
        response.inboundTokens = 1;
        response.outboundTokens = 1;
        
        bytes memory clientSignature = signRequest(request, clientPrivateKey);
        bytes memory executorSignature = signResponse(response, executorPrivateKey);
        
        accounting.processRequestSigned(
            request,
            response,
            clientSignature,
            executorSignature
        );
        
        AccountingV2.ExecutorStats memory execStats = accounting.getExecutorStats(executor);
        assertEq(execStats.totalInboundTokens, 1, "Should have 1 inbound token");
        assertEq(execStats.totalOutboundTokens, 1, "Should have 1 outbound token");
    }
    
    function testEmptyContentHashes() public {
        AccountingV2.LlmRequestCommitment memory request = createValidRequest();
        request.promptHash = keccak256(""); // Empty prompt hash
        request.systemPromptHash = bytes32(0); // Zero system prompt hash
        bytes32 requestHash = accounting.getRequestMessageHash(request);
        AccountingV2.LlmResponseCommitment memory response = createValidResponse(requestHash);
        response.contentHash = keccak256(""); // Empty response hash
        
        bytes memory clientSignature = signRequest(request, clientPrivateKey);
        
        vm.prank(executor);
        accounting.processRequest(
            request,
            response,
            clientSignature,
            "", // Empty prompt
            "", // Empty system prompt
            "" // Empty response
        );
        
        // Should succeed with empty content
        assertEq(accounting.getCurrentNonce(client), 1, "Nonce should be incremented");
    }
    
    function testFutureNonce() public {
        AccountingV2.LlmRequestCommitment memory request = createValidRequest();
        request.nonce = 10; // Future nonce (should be 1)
        bytes32 requestHash = accounting.getRequestMessageHash(request);
        AccountingV2.LlmResponseCommitment memory response = createValidResponse(requestHash);
        
        bytes memory clientSignature = signRequest(request, clientPrivateKey);
        
        vm.prank(executor);
        vm.expectRevert("Invalid nonce");
        accounting.processRequest(
            request,
            response,
            clientSignature,
            "test prompt",
            "test system prompt",
            "test response"
        );
    }
    
    function testMultipleSuccessfulRequests() public {
        // First request
        AccountingV2.LlmRequestCommitment memory request1 = createValidRequest();
        request1.nonce = 1;
        bytes32 requestHash1 = accounting.getRequestMessageHash(request1);
        AccountingV2.LlmResponseCommitment memory response1 = createValidResponse(requestHash1);
        
        bytes memory clientSignature1 = signRequest(request1, clientPrivateKey);
        bytes memory executorSignature1 = signResponse(response1, executorPrivateKey);
        
        accounting.processRequestSigned(request1, response1, clientSignature1, executorSignature1);
        
        // Second request
        AccountingV2.LlmRequestCommitment memory request2 = createValidRequest();
        request2.nonce = 2;
        bytes32 requestHash2 = accounting.getRequestMessageHash(request2);
        AccountingV2.LlmResponseCommitment memory response2 = createValidResponse(requestHash2);
        
        bytes memory clientSignature2 = signRequest(request2, clientPrivateKey);
        bytes memory executorSignature2 = signResponse(response2, executorPrivateKey);
        
        accounting.processRequestSigned(request2, response2, clientSignature2, executorSignature2);
        
        // Verify cumulative statistics
        AccountingV2.ExecutorStats memory execStats = accounting.getExecutorStats(executor);
        assertEq(execStats.requestCount, 2, "Should have 2 requests");
        assertEq(execStats.totalInboundTokens, 200, "Should have cumulative inbound tokens");
        assertEq(execStats.totalOutboundTokens, 400, "Should have cumulative outbound tokens");
        
        assertEq(accounting.getCurrentNonce(client), 2, "Nonce should be 2");
    }
    
    function testStatisticsTrackingForFailedRequest() public {
        AccountingV2.LlmRequestCommitment memory request = createValidRequest();
        bytes32 requestHash = accounting.getRequestMessageHash(request);
        AccountingV2.LlmResponseCommitment memory response = createValidResponse(requestHash);
        response.success = false; // Failed request
        
        bytes memory clientSignature = signRequest(request, clientPrivateKey);
        bytes memory executorSignature = signResponse(response, executorPrivateKey);
        
        accounting.processRequestSigned(request, response, clientSignature, executorSignature);
        
        AccountingV2.ExecutorStats memory execStats = accounting.getExecutorStats(executor);
        assertEq(execStats.requestCount, 1, "Should count failed requests");
        assertEq(execStats.successfulRequests, 0, "Should not count failed as successful");
    }
}