CREATE TABLE `ufc`.`tm_block` (
    `tm_block_id`          bigint(20) NOT NULL AUTO_INCREMENT,
    `chain_id`             varchar(100) NULL,
    `height`               varchar(100) NOT NULL,
    `time`                 varchar(100) NULL,
    `last_commit_hash`     varchar(100) NULL,
    `data_hash`            varchar(100) NULL,
    `validators_hash`      varchar(100) NULL,
    `next_validators_hash` varchar(100) NULL,
    `consensus_hash`       varchar(100) NULL,
    `app_hash`             varchar(100) NULL,
    `last_results_hash`    varchar(100) NULL,
    `evidence_hash`        varchar(100) NULL,
    `proposer_address`     varchar(100) NULL,
    PRIMARY KEY (`tm_block_id`),
    UNIQUE KEY `tm_block_UN` (`height`),
    KEY `tm_block_height_IDX` (`height`) USING BTREE
) ENGINE=InnoDB AUTO_INCREMENT=1 DEFAULT CHARSET=utf8mb4;
