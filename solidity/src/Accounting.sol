// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "./ECDSA.sol";

/**
 * @title AccountingV2
 * @dev EIP-712 compatible accounting system for Lloom decentralized LLM network
 * Implements dual signature verification, granular token accounting, and pricing transparency
 */
contract AccountingV2 {
    using ECDSA for bytes32;

    // =============================================================================
    // EIP-712 Domain and Type Hash Definitions
    // =============================================================================

    bytes32 public constant DOMAIN_TYPEHASH = keccak256(
        "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)"
    );

    bytes32 public constant LLMREQUEST_TYPEHASH = keccak256(
        "LlmRequestCommitment(address executor,string model,bytes32 promptHash,bytes32 systemPromptHash,uint32 maxTokens,uint32 temperature,uint256 inboundPrice,uint256 outboundPrice,uint64 nonce,uint64 deadline)"
    );

    bytes32 public constant LLMRESPONSE_TYPEHASH = keccak256(
        "LlmResponseCommitment(bytes32 requestHash,address client,string model,bytes32 contentHash,uint32 inboundTokens,uint32 outboundTokens,uint256 inboundPrice,uint256 outboundPrice,uint64 timestamp,bool success)"
    );

    bytes32 public immutable DOMAIN_SEPARATOR;

    string public constant DOMAIN_NAME = "Lloom Network";
    string public constant DOMAIN_VERSION = "1.0.0";

    // =============================================================================
    // Structs and Data Structures
    // =============================================================================

    struct LlmRequestCommitment {
        address executor;          // Chosen executor address
        string model;             // Model identifier  
        bytes32 promptHash;       // keccak256 of prompt content
        bytes32 systemPromptHash; // keccak256 of system prompt (or zero hash if none)
        uint32 maxTokens;         // Maximum tokens to generate
        uint32 temperature;       // Temperature * 10000 (e.g., 0.7 → 7000)
        uint256 inboundPrice;     // Price per inbound token (wei per token)
        uint256 outboundPrice;    // Price per outbound token (wei per token)
        uint64 nonce;             // Client nonce for replay protection
        uint64 deadline;          // Unix timestamp deadline
    }

    struct LlmResponseCommitment {
        bytes32 requestHash;      // Hash of the original signed request
        address client;           // Client address who made request
        string model;             // Model actually used
        bytes32 contentHash;      // keccak256 of response content
        uint32 inboundTokens;     // Actual prompt tokens consumed
        uint32 outboundTokens;    // Actual completion tokens generated
        uint256 inboundPrice;     // Price per inbound token (must match request)
        uint256 outboundPrice;    // Price per outbound token (must match request)
        uint64 timestamp;         // Execution timestamp
        bool success;             // Whether request succeeded
    }

    struct ExecutorStats {
        uint64 totalInboundTokens;
        uint64 totalOutboundTokens;  
        uint256 totalRevenue;
        uint32 requestCount;
        uint32 successfulRequests;
    }

    struct ClientStats {
        uint64 totalInboundTokens;
        uint64 totalOutboundTokens;
        uint256 totalSpent;
        uint32 requestCount;
    }

    struct DetailedUsageRecord {
        address executor;         // Executor who processed request
        address client;          // Client who made request  
        string model;            // Model used
        uint32 inboundTokens;    // Prompt tokens
        uint32 outboundTokens;   // Completion tokens
        uint256 inboundPrice;    // Price per inbound token
        uint256 outboundPrice;   // Price per outbound token
        uint256 totalCost;       // Total cost in wei
        uint64 timestamp;        // Block timestamp
        bool success;            // Request success status
    }

    // =============================================================================
    // State Variables
    // =============================================================================

    address public owner;

    // Nonce management for replay protection
    mapping(address => uint64) public clientNonces;

    // Statistics tracking
    mapping(address => ExecutorStats) public executorStats;
    mapping(address => ClientStats) public clientStats;

    // Usage records storage
    mapping(bytes32 => DetailedUsageRecord) public usageRecords;
    bytes32[] public allRequestHashes;

    // Network-wide statistics
    uint64 public totalInboundTokens;
    uint64 public totalOutboundTokens;
    uint256 public totalVolume;
    uint32 public totalRequests;

    // =============================================================================
    // Events
    // =============================================================================

    event RequestProcessed(
        bytes32 indexed requestHash,
        address indexed client,
        address indexed executor,
        string model,
        uint32 inboundTokens,
        uint32 outboundTokens,
        uint256 totalCost,
        bool success
    );

    event PriceCommitment(
        address indexed client,
        address indexed executor,
        string model,
        uint256 inboundPrice,
        uint256 outboundPrice
    );

    event NonceIncremented(
        address indexed client,
        uint64 newNonce
    );

    // =============================================================================
    // Modifiers
    // =============================================================================

    modifier onlyOwner() {
        require(msg.sender == owner, "Not the contract owner");
        _;
    }

    modifier validDeadline(uint64 deadline) {
        require(block.timestamp <= deadline, "Request deadline has passed");
        _;
    }

    // =============================================================================
    // Constructor
    // =============================================================================

    constructor() {
        owner = msg.sender;
        
        // Calculate domain separator
        DOMAIN_SEPARATOR = keccak256(abi.encode(
            DOMAIN_TYPEHASH,
            keccak256(bytes(DOMAIN_NAME)),
            keccak256(bytes(DOMAIN_VERSION)),
            block.chainid,
            address(this)
        ));
    }

    // =============================================================================
    // EIP-712 Message Hash Calculation Functions
    // =============================================================================

    function getRequestMessageHash(LlmRequestCommitment memory request) public view returns (bytes32) {
        bytes32 structHash = keccak256(abi.encode(
            LLMREQUEST_TYPEHASH,
            request.executor,
            keccak256(bytes(request.model)),
            request.promptHash,
            request.systemPromptHash,
            request.maxTokens,
            request.temperature,
            request.inboundPrice,
            request.outboundPrice,
            request.nonce,
            request.deadline
        ));

        return keccak256(abi.encodePacked(
            "\x19\x01",
            DOMAIN_SEPARATOR,
            structHash
        ));
    }

    function getResponseMessageHash(LlmResponseCommitment memory response) public view returns (bytes32) {
        bytes32 structHash = keccak256(abi.encode(
            LLMRESPONSE_TYPEHASH,
            response.requestHash,
            response.client,
            keccak256(bytes(response.model)),
            response.contentHash,
            response.inboundTokens,
            response.outboundTokens,
            response.inboundPrice,
            response.outboundPrice,
            response.timestamp,
            response.success
        ));

        return keccak256(abi.encodePacked(
            "\x19\x01",
            DOMAIN_SEPARATOR,
            structHash
        ));
    }

    // =============================================================================
    // Signature Verification Functions
    // =============================================================================

    function verifyClientSignature(
        LlmRequestCommitment memory request,
        bytes memory clientSignature
    ) internal view returns (address) {
        bytes32 messageHash = getRequestMessageHash(request);
        return messageHash.recover(clientSignature);
    }

    function verifyExecutorSignature(
        LlmResponseCommitment memory response,
        bytes memory executorSignature  
    ) internal view returns (address) {
        bytes32 messageHash = getResponseMessageHash(response);
        return messageHash.recover(executorSignature);
    }

    // =============================================================================
    // Nonce Management Functions
    // =============================================================================

    function getCurrentNonce(address client) external view returns (uint64) {
        return clientNonces[client];
    }

    function validateAndIncrementNonce(address client, uint64 providedNonce) internal {
        uint64 expectedNonce = clientNonces[client] + 1;
        require(providedNonce == expectedNonce, "Invalid nonce");
        
        clientNonces[client] = providedNonce;
        emit NonceIncremented(client, providedNonce);
    }

    // =============================================================================
    // Hash Verification Functions
    // =============================================================================

    function verifyContentHashes(
        LlmRequestCommitment memory request,
        LlmResponseCommitment memory response,
        string calldata promptContent,
        string calldata systemPromptContent,
        string calldata responseContent
    ) internal pure {
        // Verify prompt hash
        require(
            request.promptHash == keccak256(bytes(promptContent)),
            "Prompt hash mismatch"
        );

        // Verify system prompt hash (if provided)
        if (request.systemPromptHash != bytes32(0)) {
            require(
                request.systemPromptHash == keccak256(bytes(systemPromptContent)),
                "System prompt hash mismatch"
            );
        }

        // Verify response content hash
        require(
            response.contentHash == keccak256(bytes(responseContent)),
            "Response content hash mismatch"
        );
    }

    // =============================================================================
    // Main Processing Functions
    // =============================================================================

    function processRequest(
        LlmRequestCommitment calldata request,
        LlmResponseCommitment calldata response,
        bytes calldata clientSignature,
        string calldata promptContent,
        string calldata systemPromptContent,
        string calldata responseContent
    ) external validDeadline(request.deadline) {
        // Verify client signature
        address recoveredClient = verifyClientSignature(request, clientSignature);
        require(recoveredClient != address(0), "Invalid client signature");

        // Verify that the executor in the request matches the caller
        require(request.executor == msg.sender, "Executor mismatch");

        // Verify content hashes
        verifyContentHashes(request, response, promptContent, systemPromptContent, responseContent);

        // Verify response matches request
        require(response.client == recoveredClient, "Client address mismatch");
        require(
            keccak256(bytes(response.model)) == keccak256(bytes(request.model)),
            "Model mismatch"
        );
        require(response.inboundPrice == request.inboundPrice, "Inbound price mismatch");
        require(response.outboundPrice == request.outboundPrice, "Outbound price mismatch");

        // Validate and increment nonce
        validateAndIncrementNonce(recoveredClient, request.nonce);

        // Calculate request hash
        bytes32 requestHash = getRequestMessageHash(request);
        require(response.requestHash == requestHash, "Request hash mismatch");

        // Process the accounting
        _processAccounting(requestHash, request, response, recoveredClient);
    }

    function processRequestSigned(
        LlmRequestCommitment calldata request,
        LlmResponseCommitment calldata response,
        bytes calldata clientSignature,
        bytes calldata executorSignature
    ) external validDeadline(request.deadline) {
        // Verify client signature
        address recoveredClient = verifyClientSignature(request, clientSignature);
        require(recoveredClient != address(0), "Invalid client signature");

        // Verify executor signature
        address recoveredExecutor = verifyExecutorSignature(response, executorSignature);
        require(recoveredExecutor != address(0), "Invalid executor signature");

        // Verify that the executor in the request matches the signer
        require(request.executor == recoveredExecutor, "Executor mismatch");

        // Verify response matches request
        require(response.client == recoveredClient, "Client address mismatch");
        require(
            keccak256(bytes(response.model)) == keccak256(bytes(request.model)),
            "Model mismatch"
        );
        require(response.inboundPrice == request.inboundPrice, "Inbound price mismatch");
        require(response.outboundPrice == request.outboundPrice, "Outbound price mismatch");

        // Validate and increment nonce
        validateAndIncrementNonce(recoveredClient, request.nonce);

        // Calculate request hash
        bytes32 requestHash = getRequestMessageHash(request);
        require(response.requestHash == requestHash, "Request hash mismatch");

        // Process the accounting
        _processAccounting(requestHash, request, response, recoveredClient);
    }

    // =============================================================================
    // Internal Accounting Processing
    // =============================================================================

    function _processAccounting(
        bytes32 requestHash,
        LlmRequestCommitment calldata request,
        LlmResponseCommitment calldata response,
        address client
    ) internal {
        // Calculate total cost
        uint256 totalCost = (response.inboundTokens * response.inboundPrice) + 
                           (response.outboundTokens * response.outboundPrice);

        // Store detailed usage record
        usageRecords[requestHash] = DetailedUsageRecord({
            executor: request.executor,
            client: client,
            model: response.model,
            inboundTokens: response.inboundTokens,
            outboundTokens: response.outboundTokens,
            inboundPrice: response.inboundPrice,
            outboundPrice: response.outboundPrice,
            totalCost: totalCost,
            timestamp: uint64(block.timestamp),
            success: response.success
        });

        allRequestHashes.push(requestHash);

        // Update executor statistics
        ExecutorStats storage execStats = executorStats[request.executor];
        execStats.totalInboundTokens += response.inboundTokens;
        execStats.totalOutboundTokens += response.outboundTokens;
        execStats.totalRevenue += totalCost;
        execStats.requestCount += 1;
        if (response.success) {
            execStats.successfulRequests += 1;
        }

        // Update client statistics
        ClientStats storage clientStatsRef = clientStats[client];
        clientStatsRef.totalInboundTokens += response.inboundTokens;
        clientStatsRef.totalOutboundTokens += response.outboundTokens;
        clientStatsRef.totalSpent += totalCost;
        clientStatsRef.requestCount += 1;

        // Update network statistics
        totalInboundTokens += response.inboundTokens;
        totalOutboundTokens += response.outboundTokens;
        totalVolume += totalCost;
        totalRequests += 1;

        // Emit events
        emit RequestProcessed(
            requestHash,
            client,
            request.executor,
            response.model,
            response.inboundTokens,
            response.outboundTokens,
            totalCost,
            response.success
        );

        emit PriceCommitment(
            client,
            request.executor,
            request.model,
            request.inboundPrice,
            request.outboundPrice
        );
    }

    // =============================================================================
    // Query Functions
    // =============================================================================

    function getExecutorStats(address executor) external view returns (ExecutorStats memory) {
        return executorStats[executor];
    }

    function getClientStats(address client) external view returns (ClientStats memory) {
        return clientStats[client];
    }

    function getUsageRecord(bytes32 requestHash) external view returns (DetailedUsageRecord memory) {
        return usageRecords[requestHash];
    }

    function getNetworkStats() external view returns (
        uint64 _totalInboundTokens,
        uint64 _totalOutboundTokens,
        uint256 _totalVolume,
        uint32 _totalRequests
    ) {
        return (totalInboundTokens, totalOutboundTokens, totalVolume, totalRequests);
    }

    function getAllRequestHashes() external view returns (bytes32[] memory) {
        return allRequestHashes;
    }

    function getRequestCount() external view returns (uint256) {
        return allRequestHashes.length;
    }

    // =============================================================================
    // Administrative Functions
    // =============================================================================

    function transferOwnership(address newOwner) external onlyOwner {
        require(newOwner != address(0), "Invalid new owner address");
        owner = newOwner;
    }

    // =============================================================================
    // Utility Functions
    // =============================================================================

    function hasRecordedUsage(address executor) external view returns (bool) {
        return executorStats[executor].requestCount > 0;
    }

    function getExecutorSuccessRate(address executor) external view returns (uint256) {
        ExecutorStats memory stats = executorStats[executor];
        if (stats.requestCount == 0) {
            return 0;
        }
        return (stats.successfulRequests * 10000) / stats.requestCount; // Returns basis points (0-10000)
    }

    function getExecutorAverageRevenue(address executor) external view returns (uint256) {
        ExecutorStats memory stats = executorStats[executor];
        if (stats.requestCount == 0) {
            return 0;
        }
        return stats.totalRevenue / stats.requestCount;
    }

    function getClientAverageSpend(address client) external view returns (uint256) {
        ClientStats memory stats = clientStats[client];
        if (stats.requestCount == 0) {
            return 0;
        }
        return stats.totalSpent / stats.requestCount;
    }
}