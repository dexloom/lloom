// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/**
 * @title Accounting
 * @dev Records token usage for a decentralized LLM network.
 * The evidence of work is the emission of the UsageRecorded event,
 * which can be indexed and verified off-chain.
 */
contract Accounting {
    address public owner;

    event UsageRecorded(
        address indexed executor, // The address of the Executor that did the work.
        address indexed client,   // The address of the Client that requested the work.
        string model,             // The model used.
        uint256 tokenCount,       // The number of tokens processed.
        uint256 timestamp         // The block timestamp of the recording.
    );

    // Stores the total tokens processed by each executor for simple on-chain stats.
    mapping(address => uint256) public totalTokensByExecutor;
    
    // Stores the total tokens requested by each client for simple on-chain stats.
    mapping(address => uint256) public totalTokensByClient;
    
    // Total tokens processed across the entire network
    uint256 public totalTokensProcessed;

    modifier onlyOwner() {
        require(msg.sender == owner, "Not the contract owner");
        _;
    }

    constructor() {
        owner = msg.sender;
    }

    /**
     * @dev Allows an Executor to submit a record of work done.
     * The `msg.sender` is the Executor's address, providing authentication.
     * @param client The address of the client who made the request.
     * @param model The name of the model used for the job.
     * @param tokenCount The total number of tokens (prompt + completion).
     */
    function recordUsage(
        address client,
        string calldata model,
        uint256 tokenCount
    ) external {
        require(client != address(0), "Invalid client address");
        require(tokenCount > 0, "Token count must be positive");
        require(bytes(model).length > 0, "Model name cannot be empty");
        
        // Update statistics
        totalTokensByExecutor[msg.sender] += tokenCount;
        totalTokensByClient[client] += tokenCount;
        totalTokensProcessed += tokenCount;
        
        emit UsageRecorded(
            msg.sender,
            client,
            model,
            tokenCount,
            block.timestamp
        );
    }
    
    /**
     * @dev Get executor statistics
     * @param executor The executor address to query
     * @return totalTokens The total tokens processed by this executor
     */
    function getExecutorStats(address executor) external view returns (uint256 totalTokens) {
        return totalTokensByExecutor[executor];
    }
    
    /**
     * @dev Get client statistics
     * @param client The client address to query
     * @return totalTokens The total tokens requested by this client
     */
    function getClientStats(address client) external view returns (uint256 totalTokens) {
        return totalTokensByClient[client];
    }
    
    /**
     * @dev Get network-wide statistics
     * @return totalTokens The total tokens processed across the entire network
     */
    function getNetworkStats() external view returns (uint256 totalTokens) {
        return totalTokensProcessed;
    }
    
    /**
     * @dev Emergency function to update the contract owner (only current owner)
     * @param newOwner The address of the new owner
     */
    function transferOwnership(address newOwner) external onlyOwner {
        require(newOwner != address(0), "Invalid new owner address");
        owner = newOwner;
    }
    
    /**
     * @dev View function to check if an address has recorded any usage
     * @param executor The executor address to check
     * @return hasUsage True if the executor has recorded any usage
     */
    function hasRecordedUsage(address executor) external view returns (bool hasUsage) {
        return totalTokensByExecutor[executor] > 0;
    }
}