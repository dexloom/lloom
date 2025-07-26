// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test, console2} from "forge-std/Test.sol";
import {AccountingV2} from "../src/Accounting.sol";

contract AccountingV2Test is Test {
    AccountingV2 public accounting;
    
    address public client = address(0x1);
    address public executor = address(0x2);
    
    function setUp() public {
        accounting = new AccountingV2();
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
}